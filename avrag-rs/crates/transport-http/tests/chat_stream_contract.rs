use app_bootstrap::AppState;
use agent_loop::events::{AgentEvent, AgentEventSink};
use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult, AgentRunUsage};
use app_chat::agents::service::UnifiedAgentService;
use app_core::AppConfig;
use contracts::auth_runtime::{AuthContext, OrgId, SubjectKind};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateDocumentRequest, CreateNotebookRequest};
use contracts::chat::ChatEvent;
use contracts::chat::ChatResponse;
use contracts::documents::DocumentStatus;
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

struct ScriptedAgent;

#[async_trait::async_trait]
impl Agent for ScriptedAgent {
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError> {
        // Simulate RAG-specific behaviour so that RAG contract tests can verify
        // end-to-end streaming without a real runtime.
        if request.kind == agent_loop::AgentKind::Rag {
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "planning".to_string(),
                    message: "planning".to_string(),
                })
                .await;

            if request.doc_scope.is_empty() {
                let answer = "请选择一个或多个文档以继续。".to_string();
                let _ = sink
                    .emit(AgentEvent::MessageDelta {
                        text: answer.clone(),
                    })
                    .await;
                let _ = sink
                    .emit(AgentEvent::Done {
                        final_message: Some(answer.clone()),
                        usage: None,
                    })
                    .await;
                return Ok(AgentRunResult {
                    answer,
                    ..Default::default()
                });
            }

            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "retrieving".to_string(),
                    message: "retrieving".to_string(),
                })
                .await;

            return Err(common::AppError::validation(
                "rag_runtime_not_configured",
                "RAG runtime is not configured",
            ));
        }

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

#[tokio::test]
async fn post_chat_with_stream_flag_only_returns_sse() {
    let (app, workspace_id, org_id) = test_app().await;
    let request_id = "req-stream-flag";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "workspace_id": workspace_id,
                "agent_type": "chat",
                "stream": true
            }),
            None,
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(matches!(events.first(), Some(ChatEvent::Start { request_id: rid, session_id }) if rid == request_id && !session_id.is_empty()));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::AnswerStart { request_id: rid, session_id, .. } if rid == request_id && !session_id.is_empty())));
        assert!(
            events
                .iter()
                .filter(|event| matches!(event, ChatEvent::Token { request_id: rid, message_id, content } if rid == request_id && *message_id >= 0 && !content.is_empty()))
                .count()
                >= 1
        );
        assert!(matches!(events.last(), Some(ChatEvent::Done { request_id: rid, session_id, message_id, payload }) if rid == request_id && !session_id.is_empty() && *message_id >= 0 && payload.get("answer").and_then(|value| value.as_str()).is_some()));
    })
    .await;
}

#[tokio::test]
async fn post_chat_with_accept_sse_only_returns_sse() {
    let (app, workspace_id, org_id) = test_app().await;
    let request_id = "req-accept-sse";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "workspace_id": workspace_id,
                "agent_type": "chat",
                "stream": false
            }),
            Some("text/event-stream"),
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Start { request_id: rid, .. } if rid == request_id)));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::AnswerStart { request_id: rid, .. } if rid == request_id)));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Token { request_id: rid, .. } if rid == request_id)));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Done { request_id: rid, .. } if rid == request_id)));
    })
    .await;
}

#[tokio::test]
async fn post_rag_chat_stream_without_runtime_fails_closed_after_retrieval_activity() {
    let (app, workspace_id, document_id, org_id) = test_app_with_ready_document().await;
    let request_id = "req-rag-progress";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Summarize available context.",
                "workspace_id": workspace_id,
                "agent_type": "rag",
                "doc_scope": [document_id],
                "stream": true
            }),
            Some("text/event-stream"),
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        let activity_phases = events
            .iter()
            .filter_map(|event| match event {
                ChatEvent::Activity {
                    request_id: rid,
                    phase,
                    timestamp,
                    ..
                } if rid == request_id => {
                    assert!(
                        timestamp
                            .as_ref()
                            .map(|value| !value.is_empty())
                            .unwrap_or(false)
                    );
                    Some(phase.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            activity_phases,
            vec!["planning", "retrieving"]
        );
        assert!(matches!(events.last(), Some(ChatEvent::Error { request_id: rid, code, message }) if rid == request_id && code == "rag_runtime_not_configured" && !message.is_empty()));
    })
    .await;
}

#[tokio::test]
async fn post_rag_chat_with_empty_doc_scope_clarifies_without_retrieval() {
    let (app, workspace_id, org_id) = test_app().await;
    let request_id = "req-rag-empty-scope";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Summarize available context.",
                "workspace_id": workspace_id,
                "agent_type": "rag",
                "stream": true
            }),
            Some("text/event-stream"),
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(!events.iter().any(|event| matches!(
            event,
            ChatEvent::Activity { phase, .. } if phase == "retrieving"
        )));
        let streamed_answer = events
            .iter()
            .filter_map(|event| match event {
                ChatEvent::Token { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert!(streamed_answer.contains("选择"));
    })
    .await;
}

#[tokio::test]
async fn post_chat_stream_event_order_start_first_done_terminal() {
    let (app, workspace_id, org_id) = test_app().await;
    let request_id = "req-event-order";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "workspace_id": workspace_id,
                "agent_type": "chat",
                "stream": true
            }),
            None,
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(
            matches!(events.first(), Some(ChatEvent::Start { request_id: rid, .. }) if rid == request_id),
            "first SSE event must be start, got: {:?}",
            events.first()
        );
        assert!(
            matches!(events.last(), Some(ChatEvent::Done { request_id: rid, .. }) if rid == request_id),
            "last SSE event must be done, got: {:?}",
            events.last()
        );
        let done_index = events
            .iter()
            .position(|event| matches!(event, ChatEvent::Done { .. }))
            .expect("done event checked above");
        assert_eq!(
            done_index,
            events.len() - 1,
            "no events may follow done: {:?}",
            events
                .iter()
                .skip(done_index + 1)
                .map(sse_event_name)
                .collect::<Vec<_>>()
        );
    })
    .await;
}

#[tokio::test]
async fn post_chat_stream_done_payload_includes_core_fields() {
    let (app, workspace_id, org_id) = test_app().await;
    let request_id = "req-done-payload";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "workspace_id": workspace_id,
                "agent_type": "chat",
                "stream": true
            }),
            None,
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(matches!(events.first(), Some(ChatEvent::Start { .. })));
        assert!(matches!(events.last(), Some(ChatEvent::Done { .. })));

        let ChatEvent::Done { payload, .. } = events.last().unwrap() else {
            unreachable!("last event checked above");
        };
        assert!(
            payload.get("answer").and_then(|v| v.as_str()).is_some(),
            "done.payload.answer must be a string, got: {payload}"
        );
        assert!(
            payload.get("agent_type").and_then(|v| v.as_str()).is_some(),
            "done.payload.agent_type must be a string, got: {payload}"
        );
        assert!(
            payload.get("session_id").and_then(|v| v.as_str()).is_some(),
            "done.payload.session_id must be a string, got: {payload}"
        );
    })
    .await;
}

#[tokio::test]
async fn post_chat_without_streaming_returns_json() {
    let (app, workspace_id, org_id) = test_app().await;
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "workspace_id": workspace_id,
                "agent_type": "chat",
                "stream": false
            }),
            Some("application/json"),
            Some("req-json"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("application/json"))
            .unwrap_or(false)
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: ChatResponse = serde_json::from_slice(&body).unwrap();
    assert!(!payload.session_id.is_empty());
    assert!(!payload.answer.is_empty());
}

#[tokio::test]
async fn get_chat_is_not_the_streaming_entrypoint() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/chat?query=hello&stream=true")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn post_chat_stream_errors_emit_request_id_and_code() {
    let app = build_router(AppState::new(AppConfig::default()));
    let org_id = Uuid::new_v4();
    let request_id = "req-stream-error";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "agent_type": "chat",
                "stream": true
            }),
            Some("text/event-stream"),
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(matches!(events.last(), Some(ChatEvent::Error { request_id: rid, code, message }) if rid == request_id && !code.is_empty() && !message.is_empty()));
    })
    .await;
}

async fn test_app() -> (axum::Router, String, Uuid) {
    let mut state = AppState::new(AppConfig::default());
    state.set_agent_service(test_agent_service());
    let org_id = Uuid::new_v4();
    let notebook = state
        .with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User))
        .create_notebook(CreateNotebookRequest {
            name: "stream-contract".to_string(),
            description: "chat stream contract test".to_string(),
        })
        .await
        .unwrap();

    (build_router(state), notebook.id, org_id)
}

async fn test_app_with_ready_document() -> (axum::Router, String, String, Uuid) {
    let mut state = AppState::new(AppConfig::default());
    state.set_uses_memory_adapters(false);
    state.set_agent_service(test_agent_service());
    let org_id = Uuid::new_v4();
    let scoped = state.with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User));
    let notebook = scoped
        .create_notebook(CreateNotebookRequest {
            name: "stream-contract-rag".to_string(),
            description: "chat stream RAG contract test".to_string(),
        })
        .await
        .unwrap();
    let upload = scoped
        .create_document_upload(
            &notebook.id,
            CreateDocumentRequest {
                filename: "atlas.txt".to_string(),
                file_size: 32,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .unwrap();
    scoped
        .put_uploaded_document(&upload.document_id, b"atlas rollback checklist".to_vec())
        .await
        .unwrap();
    scoped
        .transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();

    (build_router(state), notebook.id, upload.document_id, org_id)
}

fn chat_post_request(
    org_id: Uuid,
    body: serde_json::Value,
    accept: Option<&str>,
    request_id: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/chat")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-org-id", org_id.to_string());

    if let Some(accept) = accept {
        builder = builder.header(header::ACCEPT, accept);
    }
    if let Some(request_id) = request_id {
        builder = builder.header("x-request-id", request_id);
    }

    builder.body(Body::from(body.to_string())).unwrap()
}

async fn assert_sse_response(
    response: axum::response::Response,
    assert_events: impl FnOnce(&[ChatEvent]),
) {
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("text/event-stream"))
            .unwrap_or(false)
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache")
    );
    assert_eq!(
        response
            .headers()
            .get("x-accel-buffering")
            .and_then(|value| value.to_str().ok()),
        Some("no")
    );

    let events = collect_sse_events(response.into_body()).await;
    assert!(!events.is_empty());
    assert_events(&events);
}

async fn collect_sse_events(body: Body) -> Vec<ChatEvent> {
    let mut body = body;
    let mut event_name = String::new();
    let mut data_lines = Vec::new();
    let mut events = Vec::new();

    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .expect("timed out waiting for SSE frame");
        let Some(frame) = frame else {
            break;
        };
        let frame = frame.expect("failed to read SSE frame");

        let Some(data) = frame.data_ref() else {
            continue;
        };

        for raw_line in String::from_utf8_lossy(data).lines() {
            let line = raw_line.trim_end_matches('\r');
            if let Some(value) = line.strip_prefix("event:") {
                event_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("data:") {
                data_lines.push(value.trim().to_string());
            } else if line.is_empty() && !data_lines.is_empty() {
                let event = serde_json::from_str::<ChatEvent>(&data_lines.join("\n"))
                    .unwrap_or_else(|_| {
                        panic!(
                            "failed to decode SSE data for event {event_name}: {}",
                            data_lines.join("\n")
                        )
                    });
                assert_eq!(sse_event_name(&event), event_name);
                events.push(event);
                event_name.clear();
                data_lines.clear();
            }
        }
    }

    if !data_lines.is_empty() {
        let event = serde_json::from_str::<ChatEvent>(&data_lines.join("\n")).unwrap();
        assert_eq!(sse_event_name(&event), event_name);
        events.push(event);
    }

    events
}

fn sse_event_name(event: &ChatEvent) -> &'static str {
    match event {
        ChatEvent::Start { .. } => "start",
        ChatEvent::OperationGuide { .. } => "operation_guide",
        ChatEvent::Activity { .. } => "activity",
        ChatEvent::AnswerStart { .. } => "answer_start",
        ChatEvent::Trace { .. } => "trace",
        ChatEvent::Token { .. } => "token",
        ChatEvent::ReasoningSummaryDelta { .. } => "reasoning_summary_delta",
        ChatEvent::Citations { .. } => "citations",
        ChatEvent::Done { .. } => "done",
        ChatEvent::Error { .. } => "error",
    }
}
