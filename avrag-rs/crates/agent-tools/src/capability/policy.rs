//! Policy Enforcement Layer — runtime permission, risk, and audit enforcement.
//!
//! Complements the v4 `guard_pipeline` (content safety) with v5 policy
//! enforcement (permissions, risk levels, external deps, rate limits).
//!
//! Core principle: **Default Deny** — any tool call must pass at least one
//! Allow rule, otherwise it is denied.

use super::{Permission, RiskLevel, ToolMetadata};

use crate::catalog::ToolCatalog;

use std::sync::OnceLock;

/// A single enforcement rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementRule {
    pub name: String,
    pub condition: EnforcementCondition,
    pub action: EnforcementAction,
}

/// Condition that triggers a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementCondition {
    /// Tool risk level is at or above the given threshold.
    RiskLevelExceeds(RiskLevel),
    /// Tool risk level is at or below the given threshold.
    RiskLevelAtMost(RiskLevel),
    /// Tool requires a specific permission (auth **has** it).
    RequiresPermission(Permission),
    /// Auth **lacks** a specific permission.
    MissingPermission(Permission),
    /// Tool involves external network access (external_deps non-empty).
    ExternalNetworkAccess,
    /// Tool is NOT in the given allow-list.
    ToolNotInAllowlist(Vec<String>),
    /// Tool ID matches one of the given IDs.
    ToolIsOneOf(Vec<String>),
    /// All sub-conditions must match (AND).
    All(Vec<EnforcementCondition>),
    /// Any sub-condition must match (OR).
    Any(Vec<EnforcementCondition>),
    /// Always matches (used for catch-all rules).
    Always,
}

/// Action taken when a rule's condition matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnforcementAction {
    /// Allow execution (only when an explicit Allow rule matches).
    Allow,
    /// Deny execution (default fallback when no rule matches).
    Deny { reason: String },
    /// Allow but record an audit log entry.
    LogOnly,
    /// Execute but mask specific output fields.
    MaskOutput { fields: Vec<String> },
    /// Require human approval before executing (interrupts the flow).
    RequireApproval { reason: String },
}

/// Runtime policy enforcer.
///
/// Implements **Default Deny**: a tool call is allowed only if at least one
/// `Allow` rule matches and no `Deny` rule matches.
#[derive(Debug, Clone, Default)]
pub struct PolicyEnforcer {
    rules: Vec<EnforcementRule>,
}

/// Process-wide cached standard rule set (built once, hot path reuses it).
static STANDARD_RULES: OnceLock<Vec<EnforcementRule>> = OnceLock::new();

impl PolicyEnforcer {
    pub fn new(rules: Vec<EnforcementRule>) -> Self {
        Self { rules }
    }

    /// Evaluate a tool call against all registered rules.
    ///
    /// Evaluation order:
    /// 1. If any Deny rule matches → Deny (explicit deny takes highest precedence).
    /// 2. If any RequireApproval rule matches → RequireApproval (interrupt flow).
    /// 3. If any Allow rule matches → Allow.
    /// 4. If a LogOnly or MaskOutput rule matches → return that action.
    /// 5. No rule matched → Default Deny.
    pub fn evaluate(
        &self,
        tool: &ToolMetadata,
        auth: Option<&contracts::auth_runtime::AuthContext>,
    ) -> EnforcementAction {
        // 1. Explicit Deny rules take precedence.
        for rule in &self.rules {
            if rule.condition.evaluate(tool, auth)
                && let EnforcementAction::Deny { reason } = &rule.action
            {
                return EnforcementAction::Deny {
                    reason: reason.clone(),
                };
            }
        }

        // 2. RequireApproval rules (interrupt flow but don't deny).
        for rule in &self.rules {
            if rule.condition.evaluate(tool, auth)
                && let EnforcementAction::RequireApproval { reason } = &rule.action
            {
                return EnforcementAction::RequireApproval {
                    reason: reason.clone(),
                };
            }
        }

        // 3. Allow rules.
        for rule in &self.rules {
            if rule.condition.evaluate(tool, auth)
                && let EnforcementAction::Allow = &rule.action
            {
                return EnforcementAction::Allow;
            }
        }

        // 4. LogOnly / MaskOutput rules.
        for rule in &self.rules {
            if rule.condition.evaluate(tool, auth) {
                match &rule.action {
                    EnforcementAction::LogOnly | EnforcementAction::MaskOutput { .. } => {
                        return rule.action.clone();
                    }
                    _ => {}
                }
            }
        }

        // 4. Default Deny.
        EnforcementAction::Deny {
            reason: "no matching allow rule (default deny)".to_string(),
        }
    }

    /// Convenience: evaluate and return true only if the result is Allow.
    pub fn is_allowed(&self, tool: &ToolMetadata, auth: Option<&contracts::auth_runtime::AuthContext>) -> bool {
        matches!(self.evaluate(tool, auth), EnforcementAction::Allow)
    }
}

impl EnforcementCondition {
    /// Check whether this condition matches the given tool and auth context.
    pub fn evaluate(&self, tool: &ToolMetadata, auth: Option<&contracts::auth_runtime::AuthContext>) -> bool {
        match self {
            EnforcementCondition::RiskLevelExceeds(threshold) => {
                risk_level_value(tool.risk_level) >= risk_level_value(*threshold)
            }
            EnforcementCondition::RiskLevelAtMost(threshold) => {
                risk_level_value(tool.risk_level) <= risk_level_value(*threshold)
            }
            EnforcementCondition::RequiresPermission(perm) => {
                auth.is_some_and(|a| a.has_permission(&permission_string(perm)))
            }
            EnforcementCondition::MissingPermission(perm) => {
                auth.is_none_or(|a| !a.has_permission(&permission_string(perm)))
            }
            EnforcementCondition::ExternalNetworkAccess => !tool.external_deps.is_empty(),
            EnforcementCondition::ToolNotInAllowlist(allowed) => !allowed.contains(&tool.id),
            EnforcementCondition::ToolIsOneOf(ids) => ids.contains(&tool.id),
            EnforcementCondition::All(conds) => conds.iter().all(|c| c.evaluate(tool, auth)),
            EnforcementCondition::Any(conds) => conds.iter().any(|c| c.evaluate(tool, auth)),
            EnforcementCondition::Always => true,
        }
    }
}

/// Build the standard set of enforcement rules used in production.
///
/// Rules (evaluated in order, but Deny always checked first inside
/// `PolicyEnforcer::evaluate`):
/// 1. Deny any permission-bearing tool unless the user holds its permission.
/// 2. Allow any permission-bearing tool when the user holds its permission.
/// 3. Allow all Low-risk tools.
/// 4. Allow all other tools (Medium risk and below — High/Critical need explicit permission).
/// 5. Default deny falls through if none of the above match.
///
/// The permission gates are **derived from the single source of truth** — the
/// [`ToolCatalog`] `permissions` field — rather than hardcoded id lists, so the
/// catalog's permission metadata can never drift from enforcement.
pub fn standard_rules() -> Vec<EnforcementRule> {
    let mut rules = Vec::new();
    for tool in ToolCatalog::standard_cached().list() {
        for perm in &tool.meta.permissions {
            let (reason, allow_name, deny_name) = match perm {
                Permission::CodeExecution => (
                    "code execution requires CodeExecution permission",
                    "allow-code-execution-with-permission",
                    "deny-code-execution-without-permission",
                ),
                Permission::ExternalNetwork => (
                    "external network access requires ExternalNetwork permission",
                    "allow-external-network-with-permission",
                    "deny-external-network-without-permission",
                ),
                _ => continue,
            };
            let id = tool.meta.id.clone();
            rules.push(EnforcementRule {
                name: format!("{deny_name}-{id}"),
                condition: EnforcementCondition::All(vec![
                    EnforcementCondition::ToolIsOneOf(vec![id.clone()]),
                    EnforcementCondition::MissingPermission(perm.clone()),
                ]),
                action: EnforcementAction::Deny {
                    reason: reason.to_string(),
                },
            });
            rules.push(EnforcementRule {
                name: format!("{allow_name}-{id}"),
                condition: EnforcementCondition::All(vec![
                    EnforcementCondition::ToolIsOneOf(vec![id]),
                    EnforcementCondition::RequiresPermission(perm.clone()),
                ]),
                action: EnforcementAction::Allow,
            });
        }
    }
    // Rule: allow all Low-risk tools unconditionally.
    rules.push(EnforcementRule {
        name: "allow-low-risk".to_string(),
        condition: EnforcementCondition::RiskLevelAtMost(RiskLevel::Low),
        action: EnforcementAction::Allow,
    });
    // Rule: allow Medium-risk tools (retrieval, weather) unconditionally.
    // In a stricter deployment this could also require a permission.
    rules.push(EnforcementRule {
        name: "allow-medium-risk".to_string(),
        condition: EnforcementCondition::RiskLevelAtMost(RiskLevel::Medium),
        action: EnforcementAction::Allow,
    });
    rules
}

/// Cached view of the standard rule set — built once via `OnceLock`, reused
/// by every dispatch so the derived rules are not recomputed on the hot path.
pub fn standard_rules_cached() -> &'static [EnforcementRule] {
    STANDARD_RULES.get_or_init(standard_rules).as_slice()
}

/// Process-wide cached standard enforcer — constructed once, injected into the
/// dispatch hot path so rules are not rebuilt on every tool call.
static STANDARD_ENFORCER: OnceLock<PolicyEnforcer> = OnceLock::new();

/// Cached standard [`PolicyEnforcer`] (Default Deny against `standard_rules`).
pub fn standard_enforcer() -> &'static PolicyEnforcer {
    STANDARD_ENFORCER.get_or_init(|| PolicyEnforcer::new(standard_rules()))
}

/// Build a permissive enforcer that allows everything (useful in tests
/// or development environments where auth is not wired up).
pub fn permissive() -> PolicyEnforcer {
    PolicyEnforcer::new(vec![EnforcementRule {
        name: "allow-all".to_string(),
        condition: EnforcementCondition::Always,
        action: EnforcementAction::Allow,
    }])
}

/// Build a strict enforcer that denies anything above Low risk unless
/// explicitly allowed by a permission-bearing rule.
pub fn strict() -> PolicyEnforcer {
    PolicyEnforcer::new(vec![
        EnforcementRule {
            name: "deny-high-risk".to_string(),
            condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::High),
            action: EnforcementAction::Deny {
                reason: "high risk tools require explicit approval".to_string(),
            },
        },
        EnforcementRule {
            name: "deny-critical-risk".to_string(),
            condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::Critical),
            action: EnforcementAction::Deny {
                reason: "critical risk tools require explicit approval".to_string(),
            },
        },
        EnforcementRule {
            name: "allow-low-risk".to_string(),
            condition: EnforcementCondition::RiskLevelAtMost(RiskLevel::Low),
            action: EnforcementAction::Allow,
        },
    ])
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Context risk level for data classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ContextRiskLevel {
    /// Internal-use data (e.g. team docs, internal APIs).
    Internal,
    /// Confidential data (e.g. user PII, financial records).
    Confidential,
    /// Publicly available data.
    Public,
}

fn risk_level_value(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 2,
        RiskLevel::High => 3,
        RiskLevel::Critical => 4,
    }
}

fn permission_string(perm: &Permission) -> String {
    match perm {
        Permission::User => "user".to_string(),
        Permission::Advanced => "advanced".to_string(),
        Permission::Admin => "admin".to_string(),
        Permission::ExternalNetwork => "external_network".to_string(),
        Permission::CodeExecution => "code_execution".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_tool(
        name: &str,
        risk: RiskLevel,
        perms: &[Permission],
        deps: &[&str],
        applicable_strategies: &[&str],
    ) -> ToolMetadata {
        ToolMetadata {
            id: name.to_string(),
            version: "1.0".to_string(),
            owner: "test".to_string(),
            description: format!("{} tool", name),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            risk_level: risk,
            permissions: perms.to_vec(),
            external_deps: deps.iter().map(|s| s.to_string()).collect(),
            deprecation: None,
            retry_policy: crate::capability::RetryPolicy::default(),
            activation_phase: crate::capability::ActivationPhase::default(),
            applicable_strategies: applicable_strategies
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    fn auth_with(permission: &str) -> contracts::auth_runtime::AuthContext {
        contracts::auth_runtime::AuthContext::new(
            contracts::auth_runtime::UserId::new(uuid::Uuid::nil()),
            contracts::auth_runtime::SubjectKind::User,
        )
        .grant(permission)
    }

    /// Expected outcome under the pre-derivation hardcoded `standard_rules`
    /// semantics, given the catalog's permission metadata. Used to lock the
    /// derived rules to the behavior they replaced.
    fn expected_hardcoded_outcome(
        tool: &ToolMetadata,
        auth: Option<&contracts::auth_runtime::AuthContext>,
    ) -> EnforcementAction {
        let has_ce = auth.is_some_and(|a| a.has_permission("code_execution"));
        let has_en = auth.is_some_and(|a| a.has_permission("external_network"));
        for perm in &tool.permissions {
            match perm {
                Permission::CodeExecution if !has_ce => {
                    return EnforcementAction::Deny {
                        reason: "code execution requires CodeExecution permission".to_string(),
                    };
                }
                Permission::ExternalNetwork if !has_en => {
                    return EnforcementAction::Deny {
                        reason: "external network access requires ExternalNetwork permission"
                            .to_string(),
                    };
                }
                _ => {}
            }
        }
        if tool.permissions.contains(&Permission::CodeExecution) && has_ce {
            return EnforcementAction::Allow;
        }
        if tool.permissions.contains(&Permission::ExternalNetwork) && has_en {
            return EnforcementAction::Allow;
        }
        match tool.risk_level {
            RiskLevel::Low | RiskLevel::Medium => EnforcementAction::Allow,
            RiskLevel::High | RiskLevel::Critical => EnforcementAction::Deny {
                reason: "no matching allow rule (default deny)".to_string(),
            },
        }
    }

    #[test]
    fn derived_rules_equivalent_to_hardcoded_semantics() {
        let enforcer = PolicyEnforcer::new(standard_rules());
        let catalog = crate::catalog::ToolCatalog::standard_cached();
        let auths: Vec<Option<contracts::auth_runtime::AuthContext>> = vec![
            None,
            Some(auth_with("user")),
            Some(auth_with("code_execution")),
            Some(auth_with("external_network")),
            Some(
                auth_with("code_execution")
                    .grant("external_network"),
            ),
        ];
        for tool in catalog.list() {
            for auth in &auths {
                let actual = enforcer.evaluate(&tool.meta, auth.as_ref());
                let expected = expected_hardcoded_outcome(&tool.meta, auth.as_ref());
                assert_eq!(
                    actual, expected,
                    "rule drift for {} (auth perm mismatch)",
                    tool.meta.id
                );
            }
        }
    }

    #[test]
    fn default_deny_blocks_unknown_tool() {
        let enforcer = PolicyEnforcer::default();
        let tool = dummy_tool(
            "unknown",
            RiskLevel::Low,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(
            matches!(action, EnforcementAction::Deny { reason } if reason.contains("default deny"))
        );
    }

    #[test]
    fn allow_rule_matches_low_risk() {
        let enforcer = PolicyEnforcer::new(standard_rules());
        let tool = dummy_tool(
            "calculator",
            RiskLevel::Low,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert_eq!(action, EnforcementAction::Allow);
    }

    #[test]
    fn web_search_denied_without_external_network_perm() {
        let enforcer = PolicyEnforcer::new(standard_rules());
        let tool = dummy_tool(
            "web_search",
            RiskLevel::High,
            &[Permission::ExternalNetwork],
            &["search-provider"],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(
            matches!(action, EnforcementAction::Deny { reason } if reason.contains("external network"))
        );
    }

    #[test]
    fn web_search_denied_even_with_wrong_perm() {
        let enforcer = PolicyEnforcer::new(standard_rules());
        let tool = dummy_tool(
            "web_search",
            RiskLevel::High,
            &[Permission::ExternalNetwork],
            &["search-provider"],
            &["chat", "rag", "search"],
        );
        let auth = auth_with("user"); // wrong permission
        let action = enforcer.evaluate(&tool, Some(&auth));
        assert!(
            matches!(action, EnforcementAction::Deny { reason } if reason.contains("external network"))
        );
    }

    #[test]
    fn code_interpreter_denied_without_code_execution_perm() {
        let enforcer = PolicyEnforcer::new(standard_rules());
        let tool = dummy_tool(
            "code_interpreter",
            RiskLevel::High,
            &[Permission::CodeExecution],
            &[],
            &["chat", "rag", "search"],
        );
        let auth = auth_with("user");
        let action = enforcer.evaluate(&tool, Some(&auth));
        assert!(
            matches!(action, EnforcementAction::Deny { reason } if reason.contains("code execution"))
        );
    }

    #[test]
    fn permissive_allows_everything() {
        let enforcer = permissive();
        let tool = dummy_tool(
            "anything",
            RiskLevel::Critical,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        assert!(enforcer.is_allowed(&tool, None));
    }

    #[test]
    fn strict_denies_high_risk() {
        let enforcer = strict();
        let tool = dummy_tool(
            "dangerous",
            RiskLevel::High,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(
            matches!(action, EnforcementAction::Deny { reason } if reason.contains("high risk"))
        );
    }

    #[test]
    fn strict_allows_low_risk() {
        let enforcer = strict();
        let tool = dummy_tool("safe", RiskLevel::Low, &[], &[], &["chat", "rag", "search"]);
        assert!(enforcer.is_allowed(&tool, None));
    }

    #[test]
    fn deny_rule_takes_precedence_over_allow() {
        let rules = vec![
            EnforcementRule {
                name: "deny-calculator".to_string(),
                condition: EnforcementCondition::ToolIsOneOf(vec!["calculator".to_string()]),
                action: EnforcementAction::Deny {
                    reason: "banned".to_string(),
                },
            },
            EnforcementRule {
                name: "allow-all-low".to_string(),
                condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::Low),
                action: EnforcementAction::Allow,
            },
        ];
        let enforcer = PolicyEnforcer::new(rules);
        let tool = dummy_tool(
            "calculator",
            RiskLevel::Low,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(matches!(action, EnforcementAction::Deny { reason } if reason == "banned"));
    }

    #[test]
    fn external_network_condition_matches() {
        let cond = EnforcementCondition::ExternalNetworkAccess;
        let tool_with = dummy_tool(
            "a",
            RiskLevel::Low,
            &[],
            &["external"],
            &["chat", "rag", "search"],
        );
        let tool_without = dummy_tool("b", RiskLevel::Low, &[], &[], &["chat", "rag", "search"]);
        assert!(cond.evaluate(&tool_with, None));
        assert!(!cond.evaluate(&tool_without, None));
    }

    #[test]
    fn tool_not_in_allowlist_blocks_unlisted() {
        let cond = EnforcementCondition::ToolNotInAllowlist(vec!["a".to_string(), "b".to_string()]);
        let tool = dummy_tool("c", RiskLevel::Low, &[], &[], &["chat", "rag", "search"]);
        assert!(cond.evaluate(&tool, None));
    }

    #[test]
    fn tool_not_in_allowlist_allows_listed() {
        let cond = EnforcementCondition::ToolNotInAllowlist(vec!["a".to_string(), "b".to_string()]);
        let tool = dummy_tool("a", RiskLevel::Low, &[], &[], &["chat", "rag", "search"]);
        assert!(!cond.evaluate(&tool, None));
    }

    #[test]
    fn require_approval_interrupts_flow() {
        let rules = vec![
            EnforcementRule {
                name: "approve-high-risk".to_string(),
                condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::Medium),
                action: EnforcementAction::RequireApproval {
                    reason: "high risk tool".to_string(),
                },
            },
            EnforcementRule {
                name: "allow-all".to_string(),
                condition: EnforcementCondition::Always,
                action: EnforcementAction::Allow,
            },
        ];
        let enforcer = PolicyEnforcer::new(rules);
        let tool = dummy_tool(
            "dangerous",
            RiskLevel::High,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(
            matches!(action, EnforcementAction::RequireApproval { reason } if reason == "high risk tool")
        );
    }

    #[test]
    fn deny_takes_precedence_over_require_approval() {
        let rules = vec![
            EnforcementRule {
                name: "deny-banned".to_string(),
                condition: EnforcementCondition::ToolIsOneOf(vec!["banned".to_string()]),
                action: EnforcementAction::Deny {
                    reason: "banned".to_string(),
                },
            },
            EnforcementRule {
                name: "approve-high-risk".to_string(),
                condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::Medium),
                action: EnforcementAction::RequireApproval {
                    reason: "high risk".to_string(),
                },
            },
        ];
        let enforcer = PolicyEnforcer::new(rules);
        let tool = dummy_tool(
            "banned",
            RiskLevel::High,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(matches!(action, EnforcementAction::Deny { reason } if reason == "banned"));
    }

    #[test]
    fn require_approval_takes_precedence_over_allow() {
        let rules = vec![
            EnforcementRule {
                name: "approve-high-risk".to_string(),
                condition: EnforcementCondition::RiskLevelExceeds(RiskLevel::Medium),
                action: EnforcementAction::RequireApproval {
                    reason: "high risk".to_string(),
                },
            },
            EnforcementRule {
                name: "allow-all".to_string(),
                condition: EnforcementCondition::Always,
                action: EnforcementAction::Allow,
            },
        ];
        let enforcer = PolicyEnforcer::new(rules);
        let tool = dummy_tool(
            "dangerous",
            RiskLevel::High,
            &[],
            &[],
            &["chat", "rag", "search"],
        );
        let action = enforcer.evaluate(&tool, None);
        assert!(
            matches!(action, EnforcementAction::RequireApproval { reason } if reason == "high risk")
        );
    }
}
