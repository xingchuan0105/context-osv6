//! Agent Audit Log — event recording for routing decisions.
//!
//! Implements the v5 audit policy:
//! - 90 days online query retention in PostgreSQL
//! - 1 year total retention (90 days online + ~275 days cold archive)

use app_documents::{AuditAction, AuditRecord};
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
// Event builders
// ---------------------------------------------------------------------------

/// Build an audit record for a routing decision.
pub fn routing_decision_record(
    org_id: &str,
    actor_id: Option<&str>,
    trace_id: &str,
    mode_id: &str,
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
            "mode_id": mode_id,
            "matched_rule": matched_rule,
            "confidence": confidence,
            "explanation": explanation,
        }),
        created_at: common::now_rfc3339(),
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
        assert_eq!(payload["mode_id"], "rag");
        assert_eq!(payload["confidence"], 0.95);
    }
}
