use super::support::*;

#[test]
fn list_workspaces_sql_sums_documents_not_status_groups() {
    let src = include_str!("../../workspaces.rs");
    assert!(
        src.contains("coalesce(sum(cnt), 0)::bigint as document_count"),
        "dashboard source count must sum per-status document rows as bigint (sum() is numeric)"
    );
    assert!(
        !src.contains("select count(*) as document_count"),
        "count(*) over grouped statuses would show 1 when every document shares a status"
    );
}

#[test]
fn ingestion_retry_backoff_is_exponential_and_capped() {
    assert_eq!(ingestion_retry_backoff_seconds(0), 30);
    assert_eq!(ingestion_retry_backoff_seconds(1), 30);
    assert_eq!(ingestion_retry_backoff_seconds(2), 60);
    assert_eq!(ingestion_retry_backoff_seconds(3), 120);
    assert_eq!(ingestion_retry_backoff_seconds(9), 3600);
}

#[test]
fn derived_document_tables_have_tenant_rls_migration() {
    let migration = include_str!("../../../../../migrations/0029_document_derived_rls.up.sql");

    for table in [
        "document_assets",
        "document_multimodal_chunks",
        "document_parse_runs",
        "document_blocks",
    ] {
        assert!(
            migration.contains(&format!("ALTER TABLE {table} ENABLE ROW LEVEL SECURITY")),
            "{table} should enable row-level security"
        );
        assert!(
            migration.contains(&format!("ALTER TABLE {table} FORCE ROW LEVEL SECURITY")),
            "{table} should force row-level security"
        );
        assert!(
            migration.contains(&format!("CREATE POLICY tenant_isolation_{table} ON {table}")),
            "{table} should have tenant isolation policy"
        );
    }
}

