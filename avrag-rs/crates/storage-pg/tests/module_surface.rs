#[test]
fn storage_pg_lib_does_not_embed_domain_query_methods() {
    let lib_rs = include_str!("../src/lib.rs");
    assert!(!lib_rs.contains("pub async fn list_workspaces("));
    assert!(!lib_rs.contains("pub async fn list_documents("));
    assert!(!lib_rs.contains("pub async fn list_messages("));
}
