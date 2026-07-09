use super::support::*;

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

