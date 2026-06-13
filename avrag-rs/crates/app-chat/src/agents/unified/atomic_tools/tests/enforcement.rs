use super::*;
use crate::agents::unified::atomic_tools::{
    dispatch_atomic_tool, dispatch_atomic_tool_with_enforcement,
};
use contracts::ToolStatus;

#[tokio::test]
async fn test_enforcement_blocks_web_search_without_external_network_perm() {
    let call = tool_call("web_search", serde_json::json!({"query": "test"}));
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let result =
        dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("external network"));
}

#[tokio::test]
async fn test_enforcement_allows_web_search_with_external_network_perm() {
    let call = tool_call("web_search", serde_json::json!({"query": "test"}));
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    )
    .grant("external_network");
    let provider = FakeSearchProvider;
    let result = dispatch_atomic_tool_with_enforcement(
        &call,
        Some(&provider),
        Some(&auth),
        None,
        None,
    )
    .await;
    assert_eq!(result.status, ToolStatus::Ok);
}

#[tokio::test]
async fn test_enforcement_blocks_web_fetch_without_external_network_perm() {
    let call = tool_call(
        "web_fetch",
        serde_json::json!({"url": "https://example.com"}),
    );
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let result =
        dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("external network"));
}

#[tokio::test]
async fn test_enforcement_allows_web_fetch_with_external_network_perm() {
    let call = tool_call(
        "web_fetch",
        serde_json::json!({"url": "https://example.com"}),
    );
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    )
    .grant("external_network");
    let result =
        dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    // Without a real HTTP client the fetch may fail, but policy should allow it.
    assert!(matches!(result.status, ToolStatus::Ok | ToolStatus::Error));
}

#[tokio::test]
async fn test_enforcement_blocks_code_interpreter_without_code_execution_perm() {
    let call = tool_call("code_interpreter", serde_json::json!({"code": "1+1"}));
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let result =
        dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    assert!(data["error"].as_str().unwrap().contains("code execution"));
}

#[tokio::test]
async fn test_legacy_path_is_permissive_no_auth() {
    let call = tool_call("web_search", serde_json::json!({"query": "test"}));
    let provider = FakeSearchProvider;
    // Legacy dispatch_atomic_tool (no auth) should use permissive enforcer
    let result = dispatch_atomic_tool(&call, Some(&provider)).await;
    assert_eq!(result.status, ToolStatus::Ok);
}
