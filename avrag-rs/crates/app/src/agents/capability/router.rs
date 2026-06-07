//! Router Layer — v5 policy-driven strategy selection.
//!
//! Replaces the hard-coded `match request.kind` in `UnifiedAgent` with a
//! rule-based resolver that is observable, deterministic, and extensible.
//!
//! Current scope: validates explicit `request.kind` selections and produces a
//! `RoutingDecision` for telemetry.  Future work: auto-routing when
//! `request.kind` is optional / absent.

use super::RiskLevel;
use crate::agents::AgentKind;
use crate::agents::runtime::AgentRequest;
use serde::{Deserialize, Serialize};

/// Classification of user query intent for auto-routing.
///
/// **Status: Reserved for future "auto mode".**
/// Currently all requests arrive with an explicit `agent_type` from the frontend
/// (Chat/Rag/Search buttons), so intent-based auto-routing is never triggered.
/// When an "auto" mode is added, replace `infer_intent` with an LLM classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Factual query that can be answered from internal documents.
    Factual,
    /// Query requiring external knowledge (news, weather, current events).
    ExternalKnowledge,
    /// Creative or open-ended conversational query.
    Creative,
    /// Code-related query.
    Code,
}

/// Infer intent from query text using keyword heuristics.
pub fn infer_intent(query: &str) -> Intent {
    let lower = query.to_lowercase();
    if lower.contains("code")
        || lower.contains("program")
        || lower.contains("function")
        || lower.contains("debug")
        || lower.contains("script")
    {
        return Intent::Code;
    }
    if lower.contains("news")
        || lower.contains("weather")
        || lower.contains("current")
        || lower.contains("latest")
        || lower.contains("today")
        || lower.contains("now")
    {
        return Intent::ExternalKnowledge;
    }
    if lower.contains("write")
        || lower.contains("create")
        || lower.contains("imagine")
        || lower.contains("story")
        || lower.contains("poem")
    {
        return Intent::Creative;
    }
    Intent::Factual
}

/// Rough token estimate: chars / 4 (good enough for routing decisions).
fn estimate_tokens(text: &str) -> u64 {
    (text.len() / 4) as u64
}

/// Collection of routing rules evaluated in priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterPolicy {
    pub rules: Vec<RouterRule>,
}

/// A single routing rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterRule {
    pub name: String,
    pub condition: RouterCondition,
    pub strategy: String,
    pub priority: u16,
    /// Whether the user can override this rule by explicitly choosing a mode.
    pub user_overridable: bool,
    /// Maximum risk level of tools this strategy would use (for tie-breaking).
    pub max_risk_level: RiskLevel,
}

/// Conditions that can be evaluated against an `AgentRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterCondition {
    /// Match the explicit agent kind.
    Kind(AgentKind),
    /// Request has a non-empty doc_scope.
    HasDocScope,
    /// Request query contains one of the given keywords.
    QueryContains(Vec<String>),
    /// Query intent matches the given classification.
    IntentClassified(Intent),
    /// Total estimated context length exceeds threshold.
    ContextLength { max_tokens: u64 },
    /// All sub-conditions must match (AND).
    All(Vec<RouterCondition>),
    /// Any sub-condition must match (OR).
    Any(Vec<RouterCondition>),
    /// Always matches (catch-all / default).
    Always,
}

/// Outcome of the routing decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub strategy_id: String,
    pub matched_rule: String,
    pub confidence: f64,
    pub overridable: bool,
    pub explanation: String,
}

impl RouterPolicy {
    /// Resolve the best strategy for the given request.
    ///
    /// Deterministic algorithm:
    /// 1. If the user explicitly chose a mode and that rule is overridable,
    ///    return the corresponding strategy.
    /// 2. Collect all matching rules.
    /// 3. Pick the highest-priority rule.
    /// 4. Tie-break by lower `max_risk_level`.
    /// 5. Final tie-break by strategy name (lexicographic, deterministic).
    pub fn resolve(&self, request: &AgentRequest) -> RoutingDecision {
        // 1. User explicit choice.
        let kind = request.kind;
        if let Some(rule) = self
            .rules
            .iter()
            .find(|r| r.user_overridable && r.condition.matches_kind(kind, request))
        {
            return RoutingDecision {
                strategy_id: rule.strategy.clone(),
                matched_rule: rule.name.clone(),
                confidence: 1.0,
                overridable: true,
                explanation: format!("user explicitly selected {:?} mode", kind),
            };
        }

        // 2. Collect all matching rules.
        let mut candidates: Vec<&RouterRule> = self
            .rules
            .iter()
            .filter(|r| r.condition.evaluate(request))
            .collect();

        if candidates.is_empty() {
            return default_routing_decision();
        }

        // 3. Highest priority.
        candidates.sort_by_key(|r| std::cmp::Reverse(r.priority));
        let max_priority = candidates[0].priority;
        candidates.retain(|r| r.priority == max_priority);

        // 4. Tie-break: lower risk level preferred.
        candidates.sort_by_key(|r| risk_level_value(r.max_risk_level));

        // 5. Deterministic tie-break: strategy name.
        candidates.sort_by(|a, b| a.strategy.cmp(&b.strategy));

        let best = candidates[0];
        RoutingDecision {
            strategy_id: best.strategy.clone(),
            matched_rule: best.name.clone(),
            confidence: 0.9,
            overridable: best.user_overridable,
            explanation: format!(
                "matched rule '{}' (priority={}, risk={:?})",
                best.name, best.priority, best.max_risk_level
            ),
        }
    }
}

impl RouterCondition {
    /// Check whether this condition matches the given request.
    pub fn evaluate(&self, request: &AgentRequest) -> bool {
        match self {
            RouterCondition::Kind(k) => request.kind == *k,
            RouterCondition::HasDocScope => !request.doc_scope.is_empty(),
            RouterCondition::QueryContains(keywords) => {
                let query_lower = request.query.to_lowercase();
                keywords.iter().any(|kw| query_lower.contains(kw))
            }
            RouterCondition::IntentClassified(intent) => infer_intent(&request.query) == *intent,
            RouterCondition::ContextLength { max_tokens } => {
                let mut total = estimate_tokens(&request.query);
                for msg in &request.messages {
                    total += estimate_tokens(&msg.content);
                }
                total > *max_tokens
            }
            RouterCondition::All(conds) => conds.iter().all(|c| c.evaluate(request)),
            RouterCondition::Any(conds) => conds.iter().any(|c| c.evaluate(request)),
            RouterCondition::Always => true,
        }
    }

    /// Shorthand for the "user explicit choice" fast-path.
    fn matches_kind(&self, kind: AgentKind, _request: &AgentRequest) -> bool {
        matches!(self, RouterCondition::Kind(k) if *k == kind)
    }
}

/// Production default policy.
pub fn standard_policy() -> RouterPolicy {
    RouterPolicy {
        rules: vec![
            RouterRule {
                name: "user-chat".to_string(),
                condition: RouterCondition::Kind(AgentKind::Chat),
                strategy: "chat".to_string(),
                priority: 100,
                user_overridable: true,
                max_risk_level: RiskLevel::Low,
            },
            RouterRule {
                name: "user-rag".to_string(),
                condition: RouterCondition::All(vec![
                    RouterCondition::Kind(AgentKind::Rag),
                    RouterCondition::HasDocScope,
                ]),
                strategy: "rag".to_string(),
                priority: 90,
                user_overridable: true,
                max_risk_level: RiskLevel::Low,
            },
            RouterRule {
                name: "user-search".to_string(),
                condition: RouterCondition::Kind(AgentKind::Search),
                strategy: "search".to_string(),
                priority: 80,
                user_overridable: true,
                max_risk_level: RiskLevel::High,
            },
            RouterRule {
                name: "auto-rag-factual".to_string(),
                condition: RouterCondition::All(vec![
                    RouterCondition::IntentClassified(Intent::Factual),
                    RouterCondition::HasDocScope,
                ]),
                strategy: "rag".to_string(),
                priority: 70,
                user_overridable: false,
                max_risk_level: RiskLevel::Low,
            },
            RouterRule {
                name: "auto-search-external".to_string(),
                condition: RouterCondition::IntentClassified(Intent::ExternalKnowledge),
                strategy: "search".to_string(),
                priority: 60,
                user_overridable: false,
                max_risk_level: RiskLevel::High,
            },
            RouterRule {
                name: "default-chat".to_string(),
                condition: RouterCondition::Always,
                strategy: "chat".to_string(),
                priority: 50,
                user_overridable: false,
                max_risk_level: RiskLevel::Low,
            },
        ],
    }
}

fn default_routing_decision() -> RoutingDecision {
    RoutingDecision {
        strategy_id: "chat".to_string(),
        matched_rule: "default".to_string(),
        confidence: 0.5,
        overridable: false,
        explanation: "no rules matched; falling back to chat".to_string(),
    }
}

fn risk_level_value(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 1,
        RiskLevel::Medium => 2,
        RiskLevel::High => 3,
        RiskLevel::Critical => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn dummy_request(kind: AgentKind, query: &str, doc_scope: Vec<String>) -> AgentRequest {
        AgentRequest {
            kind,
            query: query.to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope,
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        }
    }

    #[test]
    fn explicit_chat_routes_to_chat() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Chat, "hello", vec![]);
        let decision = policy.resolve(&req);
        assert_eq!(decision.strategy_id, "chat");
        assert_eq!(decision.matched_rule, "user-chat");
        assert!(decision.overridable);
    }

    #[test]
    fn explicit_rag_with_docscope_routes_to_rag() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Rag, "find info", vec!["doc1".to_string()]);
        let decision = policy.resolve(&req);
        assert_eq!(decision.strategy_id, "rag");
        assert_eq!(decision.matched_rule, "user-rag");
    }

    #[test]
    fn explicit_search_routes_to_search() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Search, "latest news", vec![]);
        let decision = policy.resolve(&req);
        assert_eq!(decision.strategy_id, "search");
        assert_eq!(decision.matched_rule, "user-search");
    }

    #[test]
    fn factual_query_with_docscope_auto_routes_to_rag() {
        // Simulate a request without explicit kind (would need Optional kind in future).
        // For now we test the auto-rag rule by using Chat kind + doc_scope + factual query.
        let policy = standard_policy();
        let req = dummy_request(
            AgentKind::Chat,
            "what does the pdf say",
            vec!["doc1".to_string()],
        );
        let decision = policy.resolve(&req);
        // user-chat has priority 100, so it overrides auto-rag (70).
        assert_eq!(decision.strategy_id, "chat");
        assert_eq!(decision.matched_rule, "user-chat");
    }

    #[test]
    fn auto_search_external_knowledge() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Chat, "what is the weather today", vec![]);
        let decision = policy.resolve(&req);
        // user-chat (priority 100) overrides auto-search (60)
        assert_eq!(decision.strategy_id, "chat");
    }

    #[test]
    fn default_fallback_is_chat() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Chat, "generic", vec![]);
        let decision = policy.resolve(&req);
        assert_eq!(decision.strategy_id, "chat");
    }

    #[test]
    fn routing_decision_is_deterministic() {
        let policy = standard_policy();
        let req = dummy_request(AgentKind::Search, "query", vec![]);
        let d1 = policy.resolve(&req);
        let d2 = policy.resolve(&req);
        assert_eq!(d1, d2);
    }

    #[test]
    fn query_contains_matches_keyword() {
        let cond = RouterCondition::QueryContains(vec!["weather".to_string(), "news".to_string()]);
        let req = dummy_request(AgentKind::Chat, "what is the weather", vec![]);
        assert!(cond.evaluate(&req));
    }

    #[test]
    fn query_contains_is_case_insensitive() {
        let cond = RouterCondition::QueryContains(vec!["weather".to_string()]);
        let req = dummy_request(AgentKind::Chat, "WHAT IS THE WEATHER", vec![]);
        assert!(cond.evaluate(&req));
    }

    #[test]
    fn all_condition_requires_every_subcondition() {
        let cond = RouterCondition::All(vec![
            RouterCondition::Kind(AgentKind::Rag),
            RouterCondition::HasDocScope,
        ]);
        let req_match = dummy_request(AgentKind::Rag, "q", vec!["d".to_string()]);
        let req_no_doc = dummy_request(AgentKind::Rag, "q", vec![]);
        assert!(cond.evaluate(&req_match));
        assert!(!cond.evaluate(&req_no_doc));
    }

    #[test]
    fn any_condition_requires_one_subcondition() {
        let cond = RouterCondition::Any(vec![
            RouterCondition::Kind(AgentKind::Rag),
            RouterCondition::QueryContains(vec!["search".to_string()]),
        ]);
        let req_chat = dummy_request(AgentKind::Chat, "search for x", vec![]);
        assert!(cond.evaluate(&req_chat));
    }

    #[test]
    fn intent_classified_matches_external_knowledge() {
        let cond = RouterCondition::IntentClassified(Intent::ExternalKnowledge);
        let req_weather = dummy_request(AgentKind::Chat, "what is the weather today", vec![]);
        assert!(cond.evaluate(&req_weather));
        let req_factual = dummy_request(AgentKind::Chat, "what is the capital of France", vec![]);
        assert!(!cond.evaluate(&req_factual));
    }

    #[test]
    fn intent_classified_matches_code() {
        let cond = RouterCondition::IntentClassified(Intent::Code);
        let req_code = dummy_request(AgentKind::Chat, "write a python function to sort", vec![]);
        assert!(cond.evaluate(&req_code));
        let req_chat = dummy_request(AgentKind::Chat, "hello", vec![]);
        assert!(!cond.evaluate(&req_chat));
    }

    #[test]
    fn intent_classified_matches_creative() {
        let cond = RouterCondition::IntentClassified(Intent::Creative);
        let req_creative = dummy_request(AgentKind::Chat, "write a story about dragons", vec![]);
        assert!(cond.evaluate(&req_creative));
        let req_factual = dummy_request(AgentKind::Chat, "what is 2+2", vec![]);
        assert!(!cond.evaluate(&req_factual));
    }

    #[test]
    fn context_length_matches_when_exceeded() {
        let cond = RouterCondition::ContextLength { max_tokens: 10 };
        // query "this is a very long query with many words" ~ 40 chars ~ 10 tokens
        let req_short = dummy_request(AgentKind::Chat, "hi", vec![]);
        assert!(!cond.evaluate(&req_short));
        let req_long = dummy_request(
            AgentKind::Chat,
            "this is a very long query with many words exceeding ten tokens",
            vec![],
        );
        assert!(cond.evaluate(&req_long));
    }

    #[test]
    fn auto_search_external_knowledge_via_intent() {
        let policy = standard_policy();
        // Use a query that triggers Intent::ExternalKnowledge but not user explicit kind
        let req = dummy_request(AgentKind::Chat, "what is the weather today", vec![]);
        let decision = policy.resolve(&req);
        // user-chat (priority 100) overrides auto-search-external (60)
        assert_eq!(decision.strategy_id, "chat");
    }
}
