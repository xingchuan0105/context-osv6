use app_core::{AdminAuditLogPage, AdminUsageStats};
use contracts::auth_runtime::OrgId;
use uuid::Uuid;

#[derive(serde::Serialize)]
struct AdminHealthStatus {
    status: String,
    version: String,
    uptime_secs: i64,
}

#[test]
fn health_status_serializes_expected_admin_fields() {
    let status = AdminHealthStatus {
        status: "ok".to_string(),
        version: "0.1.0".to_string(),
        uptime_secs: 3600,
    };

    let encoded = serde_json::to_value(&status).unwrap();
    assert_eq!(encoded["status"], "ok");
    assert_eq!(encoded["version"], "0.1.0");
    assert_eq!(encoded["uptime_secs"], 3600);
}

#[test]
fn audit_log_page_serializes_pagination_metadata() {
    let page = AdminAuditLogPage {
        items: Vec::new(),
        total: 0,
        page: 2,
        per_page: 50,
    };

    let encoded = serde_json::to_value(&page).unwrap();
    assert_eq!(encoded["page"], 2);
    assert_eq!(encoded["per_page"], 50);
    assert_eq!(encoded["total"], 0);
    assert!(encoded["items"].is_array());
}

#[test]
fn usage_stats_serializes_org_scoped_metrics() {
    let stats = AdminUsageStats {
        org_id: OrgId::from(Uuid::from_u128(99)),
        period: "30d".to_string(),
        query_count: 12,
        document_count: 3,
        chunk_count: 40,
        storage_bytes: 1024,
    };

    let encoded = serde_json::to_value(&stats).unwrap();
    assert_eq!(encoded["period"], "30d");
    assert_eq!(encoded["query_count"], 12);
    assert_eq!(encoded["document_count"], 3);
    assert_eq!(encoded["chunk_count"], 40);
    assert_eq!(encoded["storage_bytes"], 1024);
}
