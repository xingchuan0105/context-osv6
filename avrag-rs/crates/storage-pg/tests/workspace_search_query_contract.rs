use avrag_storage_pg::build_workspace_search_query;
use uuid::Uuid;

#[test]
fn workspace_search_query_scopes_by_org_and_orders_by_updated_at() {
    let owner_user_id = Uuid::from_u128(42);
    let sql = build_workspace_search_query(owner_user_id, None);

    assert!(sql.contains("\"workspaces\""));
    assert!(sql.contains("\"owner_user_id\""));
    assert!(sql.contains(&owner_user_id.to_string()));
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("\"updated_at\""));
}

#[test]
fn workspace_search_query_applies_title_filter_when_present() {
    let owner_user_id = Uuid::from_u128(7);
    let sql = build_workspace_search_query(owner_user_id, Some("roadmap"));

    assert!(sql.contains("LIKE"));
    assert!(sql.contains("%roadmap%"));
}
