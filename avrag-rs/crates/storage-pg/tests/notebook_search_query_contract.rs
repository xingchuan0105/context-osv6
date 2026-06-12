use avrag_storage_pg::build_notebook_search_query;
use uuid::Uuid;

#[test]
fn notebook_search_query_scopes_by_org_and_orders_by_updated_at() {
    let org_id = Uuid::from_u128(42);
    let sql = build_notebook_search_query(org_id, None);

    assert!(sql.contains("\"notebooks\""));
    assert!(sql.contains("\"org_id\""));
    assert!(sql.contains(&org_id.to_string()));
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("\"updated_at\""));
}

#[test]
fn notebook_search_query_applies_title_filter_when_present() {
    let org_id = Uuid::from_u128(7);
    let sql = build_notebook_search_query(org_id, Some("roadmap"));

    assert!(sql.contains("LIKE"));
    assert!(sql.contains("%roadmap%"));
}
