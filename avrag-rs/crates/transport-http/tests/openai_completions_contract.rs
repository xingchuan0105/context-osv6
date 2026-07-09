//! PR-3 (plan §6.1): OpenAI-compatible chat completions contract.
//!
//! The OpenAI route is `POST /v1/workspaces/{workspace_id}/chat/completions`
//! (handler `openai_chat_completions_handler` -> `chat_post_handler`). It is
//! authenticated by the same middleware as the rest of the API, so a workspace
//! API key (Bearer) with the `query` permission and a matching notebook scope
//! authorizes a completion.
//!
//! These are L6 contract cases over an in-memory `AppState` with a scripted
//! agent (no real LLM/PG), mirroring `chat_stream_contract.rs` for the
//! successful body/SSE shapes and `api_key_security_contract.rs` for the
//! 401/403 boundaries.

use app_bootstrap::AppState;
use agent_loop::events::{AgentEvent, AgentEventSink};
use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult, AgentRunUsage};
use app_chat::agents::service::UnifiedAgentService;
use app_core::AppConfig;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateApiKeyRequest, CreateWorkspaceRequest};
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

/// Scripted agent that emits a deterministic non-RAG answer without a real LLM.
struct ScriptedAgent;

#[async_trait::async_trait]
impl Agent for ScriptedAgent {
    async fn run(
        &self,
        _request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError> {
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "test".to_string(),
                message: "test agent".to_string(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: "scripted ".to_string(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: "answer".to_string(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some("scripted answer".to_string()),
                usage: None,
            })
            .await;
        Ok(AgentRunResult {
            answer: "scripted answer".to_string(),
            usage: Some(AgentRunUsage {
                provider: "test".to_string(),
                model: "scripted".to_string(),
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
                request_count: 1,
                cached_tokens: 0,
            }),
            ..Default::default()
        })
    }
}

fn test_agent_service() -> UnifiedAgentService {
    UnifiedAgentService::new(Box::new(ScriptedAgent))
}

fn test_app_state() -> AppState {
    let mut state = AppState::new(AppConfig::default());
    state.set_agent_service(test_agent_service());
    state
}

/// Build a router with a workspace notebook + a workspace API key, returning
/// `(app, workspace_id, bearer)`.
async fn create_workspace_with_key(permissions: Vec<String>) -> (axum::Router, String, String) {
    let state = test_app_state();
    let notebook = state.docs()
        .create_workspace(CreateWorkspaceRequest {
            name: "openai-contract".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions,
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .expect("api key should create");
    (build_router(state), notebook.id, key.plaintext_key)
}

/// Build a `POST /v1/workspaces/{workspace_id}/chat/completions` request with an
/// optional Bearer and a JSON body.
fn openai_chat_request(
    workspace_id: &str,
    bearer: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/v1/workspaces/{workspace_id}/chat/completions"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(bearer) = bearer {
        builder = builder.header("Authorization", format!("Bearer {bearer}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn openai_completions_without_authorization_returns_401() {
    let (app, workspace_id, _) = create_workspace_with_key(vec!["query".to_string()]).await;
    let response = app
        .oneshot(openai_chat_request(
            &workspace_id,
            None,
            serde_json::json!({ "query": "hi", "stream": false }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn openai_completions_with_workspace_key_returns_200_body() {
    let (app, workspace_id, bearer) = create_workspace_with_key(vec!["query".to_string()]).await;
    let response = app
        .oneshot(openai_chat_request(
            &workspace_id,
            Some(&bearer),
            serde_json::json!({ "query": "hi", "stream": false }),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "non-stream completion should succeed"
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    assert!(
        payload
            .get("answer")
            .and_then(|value| value.as_str())
            .is_some_and(|answer| !answer.is_empty()),
        "OpenAI completion body should carry a non-empty answer, body={payload}",
    );
}

#[tokio::test]
async fn openai_completions_with_stream_returns_sse() {
    let (app, workspace_id, bearer) = create_workspace_with_key(vec!["query".to_string()]).await;
    let response = app
        .oneshot(openai_chat_request(
            &workspace_id,
            Some(&bearer),
            serde_json::json!({ "query": "hi", "stream": true }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")),
        "stream completion should be SSE, content-type={:?}",
        response.headers().get(header::CONTENT_TYPE),
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("data:"),
        "SSE body should contain `data:` frames, body={body}",
    );
}

#[tokio::test]
async fn openai_completions_notebook_scope_mismatch_returns_403() {
    let (app, _workspace_id, bearer) = create_workspace_with_key(vec!["query".to_string()]).await;
    let other_notebook = Uuid::new_v4().to_string();
    let response = app
        .oneshot(openai_chat_request(
            &other_notebook,
            Some(&bearer),
            serde_json::json!({ "query": "hi", "stream": false }),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "workspace key scoped to one notebook must not complete against another",
    );
}
