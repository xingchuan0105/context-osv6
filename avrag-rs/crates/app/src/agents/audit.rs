//! Agent Audit Log — lifecycle management and event recording.
//!
//! Implements the v5 audit policy:
//! - 90 days online query retention in PostgreSQL
//! - 1 year total retention (90 days online + ~275 days cold archive)
//!
//! Events recorded:
//! - Routing decisions
//! - High-risk tool calls
//! - Policy deny / require-approval
//! - Budget exhaustion / degrade
//! - Permission denied

use ingestion::{AuditAction, AuditRecord};
use serde_json::Value;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Retention policy
// ---------------------------------------------------------------------------

/// Retention policy for audit logs.
pub struct AuditRetentionPolicy {
    /// Days to keep in online (fast-query) storage.
    pub online_days: i32,
    /// Days to keep in cold (archive) storage.
    pub cold_days: i32,
}

impl Default for AuditRetentionPolicy {
    fn default() -> Self {
        Self {
            online_days: 90,
            cold_days: 365 - 90, // ~275 days in cold storage after online period
        }
    }
}

impl AuditRetentionPolicy {
    /// Total retention in days (online + cold).
    pub fn total_days(&self) -> i32 {
        self.online_days + self.cold_days
    }
}

// ---------------------------------------------------------------------------
// Lifecycle manager
// ---------------------------------------------------------------------------

/// Manages audit log lifecycle: online retention, cold archiving, and pruning.
pub struct AuditLifecycleManager {
    pub policy: AuditRetentionPolicy,
}

impl AuditLifecycleManager {
    pub fn new(policy: AuditRetentionPolicy) -> Self {
        Self { policy }
    }

    pub fn with_defaults() -> Self {
        Self::new(AuditRetentionPolicy::default())
    }

    /// Prune online audit log records older than the online retention period.
    pub async fn prune_online<P: AuditStorageLegacy>(
        &self,
        storage: &P,
    ) -> Result<u64, common::AppError> {
        let deleted = storage
            .prune_audit_log(self.policy.online_days)
            .await
            .map_err(|e| common::AppError::internal(format!("Audit prune failed: {e}")))?;
        tracing::info!(
            deleted,
            retention_days = self.policy.online_days,
            "Pruned online audit logs"
        );
        Ok(deleted)
    }
}

// ---------------------------------------------------------------------------
// Storage trait (v5 — aligned with ADR-003)
// ---------------------------------------------------------------------------

/// Audit storage interface for persisting audit records.
#[async_trait::async_trait]
pub trait AuditStorage: Send + Sync {
    /// Store a single audit record.
    async fn store(&self, record: &AuditRecord) -> Result<(), String>;

    /// Query audit records by org_id and time range (unix timestamps in seconds).
    async fn query(&self, org_id: &str, start: u64, end: u64) -> Result<Vec<AuditRecord>, String>;
}

// ---------------------------------------------------------------------------
// In-memory audit storage (for testing and local dev)
// ---------------------------------------------------------------------------

/// In-memory audit storage (for testing and local dev).
pub struct InMemoryAuditStorage {
    records: std::sync::Arc<std::sync::Mutex<Vec<AuditRecord>>>,
}

impl InMemoryAuditStorage {
    pub fn new() -> Self {
        Self {
            records: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl Default for InMemoryAuditStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuditStorage for InMemoryAuditStorage {
    async fn store(&self, record: &AuditRecord) -> Result<(), String> {
        let mut guard = self.records.lock().map_err(|e| e.to_string())?;
        guard.push(record.clone());
        Ok(())
    }

    async fn query(&self, org_id: &str, start: u64, end: u64) -> Result<Vec<AuditRecord>, String> {
        let guard = self.records.lock().map_err(|e| e.to_string())?;
        let filtered: Vec<AuditRecord> = guard
            .iter()
            .filter(|r| {
                r.org_id == org_id
                    && r.created_at
                        .parse::<u64>()
                        .map(|ts| ts >= start && ts <= end)
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        Ok(filtered)
    }
}

// ---------------------------------------------------------------------------
// Legacy storage trait (for backward compatibility with existing impls)
// ---------------------------------------------------------------------------

/// Abstract storage for audit lifecycle operations.
#[async_trait::async_trait]
pub trait AuditStorageLegacy: Send + Sync {
    /// Append a single audit record to persistent storage.
    async fn append_audit_record(
        &self,
        record: &AuditRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Prune audit log records older than the retention period.
    /// Returns the number of deleted rows.
    async fn prune_audit_log(
        &self,
        retention_days: i32,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// Event builders
// ---------------------------------------------------------------------------

/// Build an audit record for a routing decision.
pub fn routing_decision_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    strategy_id: &str,
    matched_rule: &str,
    confidence: f64,
    explanation: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::RoutingDecision,
        resource_type: "agent_request".to_string(),
        resource_id: trace_id.to_string(),
        payload: serde_json::json!({
            "strategy_id": strategy_id,
            "matched_rule": matched_rule,
            "confidence": confidence,
            "explanation": explanation,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for a high-risk tool call.
pub fn high_risk_tool_call_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    tool: &str,
    risk_level: &str,
    args: &Value,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::HighRiskToolCall,
        resource_type: "tool_call".to_string(),
        resource_id: trace_id.to_string(),
        payload: serde_json::json!({
            "tool": tool,
            "risk_level": risk_level,
            "args": args,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for a policy deny decision.
pub fn policy_deny_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    resource_type: &str,
    resource_id: &str,
    reason: &str,
    rule: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::PolicyDeny,
        resource_type: resource_type.to_string(),
        resource_id: resource_id.to_string(),
        payload: serde_json::json!({
            "reason": reason,
            "rule": rule,
            "trace_id": trace_id,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for a policy require-approval decision.
pub fn policy_approval_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    resource_type: &str,
    resource_id: &str,
    reason: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::PolicyRequireApproval,
        resource_type: resource_type.to_string(),
        resource_id: resource_id.to_string(),
        payload: serde_json::json!({
            "reason": reason,
            "trace_id": trace_id,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for budget exhaustion.
pub fn budget_exhausted_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    budget_current: u8,
    budget_max: u8,
    strategy: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::BudgetExhausted,
        resource_type: "agent_run".to_string(),
        resource_id: trace_id.to_string(),
        payload: serde_json::json!({
            "budget_current": budget_current,
            "budget_max": budget_max,
            "strategy": strategy,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for a degrade event.
pub fn degrade_event_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    stage: &str,
    reason: &str,
    impact: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::DegradeEvent,
        resource_type: "agent_run".to_string(),
        resource_id: trace_id.to_string(),
        payload: serde_json::json!({
            "stage": stage,
            "reason": reason,
            "impact": impact,
        }),
        created_at: common::now_rfc3339(),
    }
}

/// Build an audit record for permission denied.
pub fn permission_denied_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    resource_type: &str,
    resource_id: &str,
    permission: &str,
    reason: &str,
) -> AuditRecord {
    AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: org_id.to_string(),
        actor_id: actor_id.map(|s| s.to_string()),
        action: AuditAction::PermissionDenied,
        resource_type: resource_type.to_string(),
        resource_id: resource_id.to_string(),
        payload: serde_json::json!({
            "permission": permission,
            "reason": reason,
            "trace_id": trace_id,
        }),
        created_at: common::now_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// Sink adapter — emit AgentEvent as audit records
// ---------------------------------------------------------------------------

/// Adapter that converts AgentEvents into audit records.
pub struct AuditSinkAdapter {
    org_id: String,
    actor_id: Option<String>,
    trace_id: String,
}

impl AuditSinkAdapter {
    pub fn new(org_id: String, actor_id: Option<String>, trace_id: String) -> Self {
        Self {
            org_id,
            actor_id,
            trace_id,
        }
    }

    /// Convert a policy deny event into an audit record.
    pub fn on_policy_deny(
        &self,
        resource_type: &str,
        resource_id: &str,
        reason: &str,
        rule: &str,
    ) -> AuditRecord {
        policy_deny_record(
            &self.org_id,
            self.actor_id.as_deref(),
            &self.trace_id,
            resource_type,
            resource_id,
            reason,
            rule,
        )
    }

    /// Convert a budget-exhausted event into an audit record.
    pub fn on_budget_exhausted(
        &self,
        budget_current: u8,
        budget_max: u8,
        strategy: &str,
    ) -> AuditRecord {
        budget_exhausted_record(
            &self.org_id,
            self.actor_id.as_deref(),
            &self.trace_id,
            budget_current,
            budget_max,
            strategy,
        )
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl AuditStorageLegacy for avrag_storage_pg::PgAppRepository {
    async fn append_audit_record(
        &self,
        record: &AuditRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.append_audit_record(record)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn prune_audit_log(
        &self,
        retention_days: i32,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        self.prune_audit_log(retention_days)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_retention_policy() {
        let policy = AuditRetentionPolicy::default();
        assert_eq!(policy.online_days, 90);
        assert_eq!(policy.total_days(), 365);
    }

    #[test]
    fn routing_decision_record_has_correct_action() {
        let record = routing_decision_record(
            "org-1",
            Some("user-1"),
            "trace-1",
            "rag",
            "doc_scope_present",
            0.95,
            "doc_scope is non-empty",
        );
        assert_eq!(record.action, AuditAction::RoutingDecision);
        assert_eq!(record.org_id, "org-1");
        assert_eq!(record.actor_id, Some("user-1".to_string()));
        assert_eq!(record.resource_id, "trace-1");
        let payload = record.payload.as_object().unwrap();
        assert_eq!(payload["strategy_id"], "rag");
        assert_eq!(payload["confidence"], 0.95);
    }

    #[test]
    fn high_risk_tool_call_record_builds() {
        let record = high_risk_tool_call_record(
            "org-1",
            None,
            "trace-2",
            "code_interpreter",
            "high",
            &serde_json::json!({"code": "1+1"}),
        );
        assert_eq!(record.action, AuditAction::HighRiskToolCall);
        assert_eq!(record.resource_type, "tool_call");
    }

    #[test]
    fn budget_exhausted_record_builds() {
        let record = budget_exhausted_record("org-1", None, "trace-3", 4, 4, "rag");
        assert_eq!(record.action, AuditAction::BudgetExhausted);
        let payload = record.payload.as_object().unwrap();
        assert_eq!(payload["budget_current"], 4);
        assert_eq!(payload["budget_max"], 4);
    }

    #[test]
    fn degrade_event_record_builds() {
        let record =
            degrade_event_record("org-1", None, "trace-4", "retrieve", "timeout", "skipped");
        assert_eq!(record.action, AuditAction::DegradeEvent);
        let payload = record.payload.as_object().unwrap();
        assert_eq!(payload["stage"], "retrieve");
    }

    #[test]
    fn permission_denied_record_builds() {
        let record = permission_denied_record(
            "org-1",
            Some("user-2"),
            "trace-5",
            "document",
            "doc-123",
            "read",
            "insufficient clearance",
        );
        assert_eq!(record.action, AuditAction::PermissionDenied);
        assert_eq!(record.actor_id, Some("user-2".to_string()));
    }

    #[test]
    fn audit_sink_adapter_on_policy_deny() {
        let adapter = AuditSinkAdapter::new("org-1".to_string(), None, "trace-6".to_string());
        let record = adapter.on_policy_deny("tool_call", "call-1", "risk_too_high", "risk_rule_7");
        assert_eq!(record.action, AuditAction::PolicyDeny);
        assert_eq!(record.org_id, "org-1");
    }
}
