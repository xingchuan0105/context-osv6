//! PR-2 (plan §5.2): MCP / API key authorization boundary at the integration
//! layer — real PG auth store + full router. These mirror the L6 contract cases
//! in `mcp_unified_contract.rs` / `api_key_security_contract.rs` but exercise
//! the real integration stack instead of an in-memory `AppState`.
//!
//! Each MCP `tools/call` rejection is returned as a JSON-RPC error envelope
//! (HTTP 200, `error.data.error` = the AppError code); REST rejections return
//! HTTP 403 with `{"error": "<code>"}`.

use std::time::Duration;

use common::CreateApiKeyRequest;
use contracts::agent_permissions::PERM_ADMIN;
use serde_json::{Value, json};

use super::mcp_agent_flow::mcp_tools_call;
use crate::product_e2e::TestContext;

/// Read the AppError code from a JSON-RPC error envelope (`error.data.error`).
fn mcp_error_code(payload: &Value) -> Option<&str> {
    payload
        .pointer("/error/data/error")
        .and_then(|v| v.as_str())
}

/// Create a workspace-scoped API key and return its plaintext bearer token.
async fn workspace_key(ctx: &TestContext, workspace_id: &str, permissions: &[&str]) -> String {
    let state = ctx
        .app_state
        .as_ref()
        .expect("app_state present in integration profile")
        .clone();
    let key = state.admin_api()
        .create_api_key(
            workspace_id,
            CreateApiKeyRequest {
                name: "boundary".to_string(),
                permissions: permissions.iter().map(|p| p.to_string()).collect(),
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .expect("create workspace api key");
    key.plaintext_key
}

/// Create an org-scoped API key (requires admin on the bootstrap auth) and
/// return its plaintext bearer token.
async fn org_key(ctx: &TestContext, permissions: &[&str]) -> String {
    let state = ctx
        .app_state
        .as_ref()
        .expect("app_state present in integration profile")
        .clone();
    let admin_state = state.with_auth(state.auth().clone().grant(PERM_ADMIN));
    let key = admin_state
        .admin_api()
        .create_org_api_key(CreateApiKeyRequest {
            name: "org-boundary".to_string(),
            permissions: permissions.iter().map(|p| p.to_string()).collect(),
            rate_limit_rpm: Some(60),
            expires_at: None,
        })
        .await
        .expect("create org api key");
    key.plaintext_key
}

#[tokio::test]
async fn workspace_key_cannot_call_org_mcp_tool() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("boundary-ws").await.unwrap();
    let bearer = workspace_key(&ctx, &notebook.id, &["query"]).await;

    let (status, payload) = mcp_tools_call(&ctx, &bearer, "org.list_workspaces", json!({})).await;
    assert_eq!(status, 200, "body: {payload}");
    assert_eq!(
        mcp_error_code(&payload),
        Some("workspace_key_cannot_call_org_tools")
    );
}

#[tokio::test]
async fn org_key_cannot_call_workspace_mcp_tool() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;
    let bearer = org_key(&ctx, &["query"]).await;

    let (status, payload) = mcp_tools_call(
        &ctx,
        &bearer,
        "workspace.rag_query",
        json!({ "workspace_id": uuid::Uuid::new_v4().to_string(), "query": "x" }),
    )
    .await;
    assert_eq!(status, 200, "body: {payload}");
    assert_eq!(
        mcp_error_code(&payload),
        Some("org_key_cannot_call_workspace_tools")
    );
}

#[tokio::test]
async fn workspace_key_cannot_query_other_workspace() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("boundary-scope").await.unwrap();
    let bearer = workspace_key(&ctx, &notebook.id, &["query"]).await;

    let (status, payload) = mcp_tools_call(
        &ctx,
        &bearer,
        "workspace.rag_query",
        json!({ "workspace_id": uuid::Uuid::new_v4().to_string(), "query": "hello" }),
    )
    .await;
    assert_eq!(status, 200, "body: {payload}");
    assert_eq!(mcp_error_code(&payload), Some("notebook_scope_mismatch"));
}

#[tokio::test]
async fn api_key_cannot_list_workspace_notes() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("boundary-notes").await.unwrap();
    let bearer = workspace_key(&ctx, &notebook.id, &["query"]).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("notes reqwest client");
    let resp = client
        .get(format!(
            "{}/api/v1/workspaces/{}/notes",
            ctx.base_url, notebook.id
        ))
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
        .expect("notes GET send");
    assert_eq!(resp.status().as_u16(), 403);
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    assert_eq!(
        body.get("error").and_then(|v| v.as_str()),
        Some("api_key_forbidden")
    );
}
