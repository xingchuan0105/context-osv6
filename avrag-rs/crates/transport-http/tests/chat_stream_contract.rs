use app::{AppConfig, AppState};
use avrag_auth::{AuthContext, OrgId, SubjectKind};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{ChatResponse, CreateNotebookRequest};
use contracts::chat::ChatEvent;
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

#[tokio::test]
async fn post_chat_with_stream_flag_only_returns_sse() {
    let (app, notebook_id, org_id) = test_app().await;
    let request_id = "req-stream-flag";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "notebook_id": notebook_id,
                "agent_type": "general",
                "stream": true
            }),
            None,
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(matches!(events.first(), Some(ChatEvent::Start { request_id: rid, session_id }) if rid == request_id && !session_id.is_empty()));
        assert!(matches!(events.get(1), Some(ChatEvent::Token { request_id: rid, message_id, content }) if rid == request_id && *message_id >= 0 && !content.is_empty()));
        assert!(matches!(events.last(), Some(ChatEvent::Done { request_id: rid, session_id, message_id, payload }) if rid == request_id && !session_id.is_empty() && *message_id >= 0 && payload.get("answer").and_then(|value| value.as_str()).is_some()));
    })
    .await;
}

#[tokio::test]
async fn post_chat_with_accept_sse_only_returns_sse() {
    let (app, notebook_id, org_id) = test_app().await;
    let request_id = "req-accept-sse";
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "notebook_id": notebook_id,
                "agent_type": "general",
                "stream": false
            }),
            Some("text/event-stream"),
            Some(request_id),
        ))
        .await
        .unwrap();

    assert_sse_response(response, |events| {
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Start { request_id: rid, .. } if rid == request_id)));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Token { request_id: rid, .. } if rid == request_id)));
        assert!(events.iter().any(|event| matches!(event, ChatEvent::Done { request_id: rid, .. } if rid == request_id)));
    })
    .await;
}

#[tokio::test]
async fn post_chat_without_streaming_returns_json() {
    let (app, notebook_id, org_id) = test_app().await;
    let response = app
        .oneshot(chat_post_request(
            org_id,
            serde_json::json!({
                "query": "Reply with a short answer.",
                "notebook_id": notebook_id,
                "agent_type": "general",
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
                "agent_type": "general",
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
    let state = AppState::new(AppConfig::default());
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
                let event =
                    serde_json::from_str::<ChatEvent>(&data_lines.join("\n")).unwrap_or_else(|_| {
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
        ChatEvent::Trace { .. } => "trace",
        ChatEvent::Token { .. } => "token",
        ChatEvent::Citations { .. } => "citations",
        ChatEvent::Done { .. } => "done",
        ChatEvent::Error { .. } => "error",
    }
}
