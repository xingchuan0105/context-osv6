use app_bootstrap::AppState;
use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use common::{AppError};
use contracts::{ExecutePlanRequest, RuntimeExecuteRequest};
use contracts::chat::{ChatRequest};
use contracts::documents::{CitationLookupRequest};
use contracts::notebooks::{CreateChatSessionRequest, UpdateChatSessionRequest};
use contracts::chat::ChatEvent;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::RequestState;
use super::{app_error_response, error_response};

pub(crate) async fn rag_execute_plan_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    payload: Result<Json<ExecutePlanRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(req) = match payload {
        Ok(payload) => payload,
        Err(error) => {
            return app_error_response(AppError::validation(
                "invalid_execute_plan",
                format!("invalid execute-plan JSON: {error}"),
            ));
        }
    };
    match state.execute_rag_execute_plan(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn runtime_execute_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    payload: Result<Json<RuntimeExecuteRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(req) = match payload {
        Ok(payload) => payload,
        Err(error) => {
            return app_error_response(AppError::validation(
                "invalid_runtime_execute",
                format!("invalid runtime execute JSON: {error}"),
            ));
        }
    };
    match state.execute_runtime_tools(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => app_error_response(error),
    }
}

#[tracing::instrument(skip(state, headers), fields(agent_type = %req.agent_type, request_id = tracing::field::Empty))]
pub(crate) async fn chat_post_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    let should_stream = req.stream || accepts_sse(&headers);
    let source_type = req.source_type.clone();
    let notebook_id = req
        .notebook_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let agent_type = req.agent_type.clone();
    let query_len = req.query.len();
    let surface = if source_type.as_deref() == Some("share") {
        analytics::Surface::SharedKb
    } else {
        analytics::Surface::Workspace
    };
    let request_id = state
        .auth()
        .request_id()
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    tracing::Span::current().record("request_id", &request_id);

    let started_event = if source_type.as_deref() == Some("share") {
        analytics::ProductEventName::SharedKbChatStarted
    } else if agent_type == "search" {
        analytics::ProductEventName::SearchStarted
    } else {
        analytics::ProductEventName::ChatStarted
    };
    state
        .record_product_event_if_available(
            started_event,
            surface,
            analytics::ResultTag::Success,
            None,
            notebook_id,
            serde_json::json!({
                "agent_type": agent_type,
                "query_length": query_len,
                "stream": should_stream,
            }),
        )
        .await;

    if should_stream {
        return chat_live_stream_response(
            state,
            req,
            request_id,
            surface,
            notebook_id,
            agent_type,
            query_len,
        );
    }

    match state.execute_chat(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            let event_name = chat_failure_event_name(&agent_type);
            state
                .record_product_event_if_available(
                    event_name,
                    surface,
                    analytics::ResultTag::Failure,
                    None,
                    notebook_id,
                    serde_json::json!({
                        "agent_type": agent_type,
                        "error_code": e.code(),
                        "query_length": query_len,
                    }),
                )
                .await;
            app_error_response(e)
        }
    }
}

fn accepts_sse(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|item| item.trim() == "text/event-stream")
        })
        .unwrap_or(false)
}

fn chat_live_stream_response(
    state: AppState,
    req: ChatRequest,
    request_id: String,
    surface: analytics::Surface,
    notebook_id: Option<Uuid>,
    agent_type: String,
    query_len: usize,
) -> Response {
    let (sender, receiver) = unbounded_channel();
    let request_id_for_task = request_id.clone();
    let agent_type_for_task = agent_type.clone();

    // Shared cancellation token: SseStreamGuard cancels it on stream drop
    // (which happens when the client disconnects), and execute_chat_stream
    // observes it via AgentRequest.cancellation_token to stop work early.
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();

    tokio::spawn(async move {
        let error_sender = sender.clone();
        if let Err(error) = state
            .execute_chat_stream(req, request_id_for_task.clone(), sender, cancel_for_task)
            .await
        {
            state
                .record_product_event_if_available(
                    chat_failure_event_name(&agent_type_for_task),
                    surface,
                    analytics::ResultTag::Failure,
                    None,
                    notebook_id,
                    serde_json::json!({
                        "agent_type": agent_type_for_task,
                        "error_code": error.code(),
                        "query_length": query_len,
                    }),
                )
                .await;
            let _ = error_sender.send(ChatEvent::Error {
                request_id: request_id_for_task,
                code: error.code().to_string(),
                message: error.message().to_string(),
            });
        }
    });

    sse_response_from_receiver(receiver, surface_label(surface), cancel)
}

fn sse_response_from_receiver(
    mut receiver: UnboundedReceiver<ChatEvent>,
    surface: &'static str,
    cancel: CancellationToken,
) -> Response {
    let stream = async_stream::stream! {
        let _guard = SseStreamGuard(surface, cancel);
        telemetry::prometheus::inc_sse_streams(surface);

        while let Some(event) = receiver.recv().await {
            let event_name = sse_event_name(&event);
            telemetry::prometheus::observe_sse_event(surface, event_name);
            yield Ok::<_, Infallible>(sse_event(event_name, &event));
        }
    };

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();
    add_sse_headers(&mut response);
    response
}

fn sse_event(event_name: &str, payload: &ChatEvent) -> Event {
    Event::default()
        .event(event_name)
        .data(serde_json::to_string(payload).unwrap_or_default())
}

fn sse_event_name(event: &ChatEvent) -> &'static str {
    match event {
        ChatEvent::Start { .. } => "start",
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

fn chat_failure_event_name(agent_type: &str) -> analytics::ProductEventName {
    if agent_type == "search" {
        analytics::ProductEventName::SearchFailed
    } else {
        analytics::ProductEventName::ChatFailed
    }
}

fn add_sse_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
}

fn surface_label(surface: analytics::Surface) -> &'static str {
    match surface {
        analytics::Surface::SharedKb => "shared_kb",
        _ => "workspace",
    }
}

struct SseStreamGuard(&'static str, CancellationToken);

impl Drop for SseStreamGuard {
    fn drop(&mut self) {
        telemetry::prometheus::dec_sse_streams(self.0);
        self.1.cancel();
    }
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct SearchQueryParams {
    pub q: String,
    #[allow(dead_code)]
    pub scope: Option<String>,
}

pub(crate) async fn search_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<SearchQueryParams>,
) -> Response {
    let (notebooks, sessions, sources) = state.search(&params.q).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "notebooks": notebooks,
            "sessions": sessions,
            "sources": sources,
        })),
    )
        .into_response()
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ChatSessionsQuery {
    pub notebook_id: Option<String>,
}

pub(crate) async fn list_chat_sessions_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ChatSessionsQuery>,
) -> Response {
    let sessions = state.list_sessions(params.notebook_id.as_deref()).await;
    (
        StatusCode::OK,
        Json(contracts::notebooks::ChatSessionListResponse { sessions }),
    )
        .into_response()
}
pub(crate) async fn create_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CreateChatSessionRequest>,
) -> Response {
    match state.create_session(req).await {
        Ok(session) => (StatusCode::CREATED, Json(session)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.get_session(&session_id).await {
        Some(session) => (StatusCode::OK, Json(session)).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "session_not_found",
            "session not found",
        ),
    }
}

pub(crate) async fn update_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
    Json(req): Json<UpdateChatSessionRequest>,
) -> Response {
    match state.update_session(&session_id, req).await {
        Ok(session) => (StatusCode::OK, Json(session)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn delete_chat_session_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.delete_session(&session_id).await {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn get_chat_messages_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.list_messages(&session_id).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(contracts::chat::ChatMessageListResponse { messages }),
        )
            .into_response(),
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn citation_lookup_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<CitationLookupRequest>,
) -> Response {
    match state
        .lookup_citation(&req.session_id, req.message_id, req.citation_id)
        .await
    {
        Ok(detail) => {
            let metadata = serde_json::json!({
                "message_id": req.message_id,
                "citation_id": req.citation_id,
                "doc_id": detail.doc_id.clone(),
                "chunk_id": detail.chunk_id.clone(),
                "page": detail.page,
            });
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::CitationOpened,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    uuid::Uuid::parse_str(&req.session_id).ok(),
                    None,
                    metadata.clone(),
                )
                .await;
            state
                .record_product_event_if_available(
                    analytics::ProductEventName::SourceFocused,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    uuid::Uuid::parse_str(&req.session_id).ok(),
                    None,
                    metadata,
                )
                .await;
            (StatusCode::OK, Json(detail)).into_response()
        }
        Err(error) => app_error_response(error),
    }
}

pub(crate) async fn citation_asset_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(asset_id): Path<String>,
) -> Response {
    match state.get_citation_asset(&asset_id).await {
        Ok((bytes, mime_type)) => {
            (StatusCode::OK, [(header::CONTENT_TYPE, mime_type)], bytes).into_response()
        }
        Err(error) => app_error_response(error),
    }
}
pub(crate) async fn message_feedback_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<contracts::chat::MessageFeedbackRequest>,
) -> Response {
    let rating = match req.rating {
        contracts::chat::MessageFeedbackRating::Up => "up",
        contracts::chat::MessageFeedbackRating::Down => "down",
    };
    let metadata = serde_json::json!({
        "message_id": req.message_id,
        "rating": rating,
    });
    state
        .record_product_event_if_available(
            analytics::ProductEventName::MessageFeedback,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            uuid::Uuid::parse_str(&req.session_id).ok(),
            None,
            metadata,
        )
        .await;
    (StatusCode::OK, Json(contracts::auth::EmptyResponse {})).into_response()
}
// ---------------------------------------------------------------------------
// Agent capabilities
// ---------------------------------------------------------------------------

pub(crate) async fn agent_capabilities_handler() -> Response {
    let response = app_chat::agents::capability::build_capabilities_response();
    (StatusCode::OK, Json(response)).into_response()
}
