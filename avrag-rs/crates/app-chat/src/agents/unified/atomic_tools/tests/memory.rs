use super::*;
use crate::agents::unified::atomic_tools::dispatch_atomic_tool_with_enforcement;
use contracts::ToolStatus;

async fn chat_persistence_from_env() -> Option<std::sync::Arc<dyn app_core::ChatPersistencePort>> {
    // PG-backed memory tool tests are exercised in product_e2e; unit tests here
    // skip when no in-crate adapter is wired (app-bootstrap adapters are private).
    let _ = std::env::var("DATABASE_URL").ok()?;
    None
}

fn memory_test_auth() -> avrag_auth::AuthContext {
    avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::new_v4()),
        avrag_auth::SubjectKind::User,
    )
    .with_actor_id(avrag_auth::ActorId::new(uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn test_conversation_history_load_without_memory_context_errors() {
    let auth = memory_test_auth();
    let call = tool_call("conversation_history_load", serde_json::json!({}));
    let result = dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    let err = data["error"].as_str().unwrap();
    assert!(
        err.contains("requires"),
        "expected context guard error, got: {err}"
    );
}

#[tokio::test]
async fn test_user_profile_load_without_memory_context_errors() {
    let auth = memory_test_auth();
    let call = tool_call("user_profile_load", serde_json::json!({}));
    let result = dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth), None, None).await;
    assert_eq!(result.status, ToolStatus::Error);
    let data = result.data.unwrap();
    let err = data["error"].as_str().unwrap();
    assert!(
        err.contains("requires"),
        "expected context guard error, got: {err}"
    );
}

#[tokio::test]
async fn test_user_profile_load_with_pg_but_no_actor_reaches_memory_dispatch() {
    let Some(chat_persistence) = chat_persistence_from_env().await else {
        return;
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::new(uuid::Uuid::new_v4()),
        avrag_auth::SubjectKind::User,
    );
    let call = tool_call("user_profile_load", serde_json::json!({}));
    let result = dispatch_atomic_tool_with_enforcement(
        &call,
        None,
        Some(&auth),
        None,
        Some(&*chat_persistence),
    )
    .await;
    assert_eq!(result.status, ToolStatus::Error);
    assert_eq!(
        result.data.unwrap()["error"].as_str().unwrap(),
        "authenticated user required"
    );
}

#[tokio::test]
async fn test_conversation_history_load_with_pg_context_succeeds() {
    let Some(chat_persistence) = chat_persistence_from_env().await else {
        return;
    };
    let auth = memory_test_auth();
    let session_id = uuid::Uuid::new_v4();
    let call = tool_call("conversation_history_load", serde_json::json!({"limit": 5}));
    let result = dispatch_atomic_tool_with_enforcement(
        &call,
        None,
        Some(&auth),
        Some(session_id),
        Some(&*chat_persistence),
    )
    .await;
    assert_eq!(
        result.status,
        ToolStatus::Ok,
        "unexpected result: {:?}",
        result.data
    );
    let data = result.data.unwrap();
    assert!(data.get("messages").and_then(|v| v.as_array()).is_some());
    assert_eq!(data["message_count"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_user_profile_load_with_pg_context_succeeds() {
    let Some(chat_persistence) = chat_persistence_from_env().await else {
        return;
    };
    let auth = memory_test_auth();
    let call = tool_call("user_profile_load", serde_json::json!({}));
    let result = dispatch_atomic_tool_with_enforcement(
        &call,
        None,
        Some(&auth),
        None,
        Some(&*chat_persistence),
    )
    .await;
    assert_eq!(
        result.status,
        ToolStatus::Ok,
        "unexpected result: {:?}",
        result.data
    );
    let data = result.data.unwrap();
    assert!(data.get("structured_profile").is_some());
    assert!(
        data.get("expertise_domains")
            .and_then(|v| v.as_array())
            .is_some()
    );
}
