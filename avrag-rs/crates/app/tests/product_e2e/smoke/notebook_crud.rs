//! Notebook create / read / update / delete over HTTP (Product E2E).

use crate::product_e2e::TestContext;

#[tokio::test]
async fn notebook_crud_lifecycle_via_http() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;

    let created = ctx.create_notebook("crud-original").await.unwrap();
    let workspace_id = created.id.clone();

    let get_resp = ctx.get_notebook(&workspace_id).await.unwrap();
    assert_eq!(get_resp.status, 200, "get notebook: {get_resp:?}");
    assert_eq!(
        get_resp.body_json["notebook"]["name"].as_str(),
        Some("crud-original")
    );

    let list_resp = ctx.list_notebooks().await.unwrap();
    assert_eq!(list_resp.status, 200, "list notebooks: {list_resp:?}");
    let workspaces = list_resp.body_json["notebooks"]
        .as_array()
        .expect("notebooks array");
    assert!(
        notebooks
            .iter()
            .any(|nb| nb.get("id").and_then(|v| v.as_str()) == Some(workspace_id.as_str())),
        "created notebook should appear in list"
    );

    let update_resp = ctx
        .update_notebook(&workspace_id, "crud-renamed", "updated description")
        .await
        .unwrap();
    assert_eq!(update_resp.status, 200, "update notebook: {update_resp:?}");
    assert_eq!(
        update_resp.body_json["notebook"]["name"].as_str(),
        Some("crud-renamed")
    );

    let delete_resp = ctx.delete_notebook(&workspace_id).await.unwrap();
    assert_eq!(delete_resp.status, 200, "delete notebook: {delete_resp:?}");

    let gone_resp = ctx.get_notebook(&workspace_id).await.unwrap();
    assert!(
        gone_resp.status == 404 || gone_resp.status == 403,
        "deleted notebook should be inaccessible, got HTTP {}",
        gone_resp.status
    );
}
