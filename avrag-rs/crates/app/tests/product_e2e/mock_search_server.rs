//! Mock Brave Search HTTP server for Product E2E.

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    extract::Query,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Runtime toggles for the mock Brave Search server.
#[derive(Clone)]
pub(crate) struct MockSearchControls {
    pub should_429: Arc<AtomicBool>,
    pub should_empty: Arc<AtomicBool>,
    pub delay_ms: Arc<AtomicU64>,
}

impl MockSearchControls {
    pub fn new() -> Self {
        Self {
            should_429: Arc::new(AtomicBool::new(false)),
            should_empty: Arc::new(AtomicBool::new(false)),
            delay_ms: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Start a mock Brave Search HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, controls).
pub(crate) async fn start_mock_search_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    MockSearchControls,
) {
    let controls = MockSearchControls::new();
    let flag = controls.clone();
    let flag2 = controls.clone();
    let flag3 = controls.clone();
    let app = Router::new()
        .route(
            "/res/v1/llm/context",
            post(move |req| mock_search_handler(req, flag.clone())),
        )
        .route(
            "/res/v1/news/search",
            get(move |Query(params): Query<MockNewsQuery>| async move {
                mock_news_search_handler(params, flag2.clone())
            })
            .post(move |req| mock_search_handler(req, flag3.clone())),
        );

    let (listener, base_url) = bind_persistent_listener().await;

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, controls)
}

#[derive(Debug, serde::Deserialize)]
struct MockNewsQuery {
    q: Option<String>,
}

fn mock_news_search_handler(
    params: MockNewsQuery,
    controls: MockSearchControls,
) -> axum::response::Response {
    if controls.should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    if controls.should_empty.load(Ordering::SeqCst) {
        return Json(json!({ "results": [] })).into_response();
    }

    let _query = params.q.as_deref().unwrap_or("unknown");
    Json(json!({
        "results": [
            {
                "title": "Tokyo Weather Today",
                "url": "https://example.com/weather-tokyo",
                "description": "Sunny with a high of 25°C in Tokyo today."
            }
        ]
    }))
    .into_response()
}

async fn mock_search_handler(
    Json(req): Json<serde_json::Value>,
    controls: MockSearchControls,
) -> axum::response::Response {
    if controls.should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let delay_ms = controls.delay_ms.load(Ordering::SeqCst);
    if delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    if controls.should_empty.load(Ordering::SeqCst) {
        return Json(json!({
            "grounding": { "generic": [], "map": [] },
            "sources": {}
        }))
        .into_response();
    }

    let _query = req["q"].as_str().unwrap_or("unknown");
    Json(json!({
        "grounding": {
            "generic": [
                {
                    "url": "https://example.com/weather-tokyo",
                    "title": "Tokyo Weather Today",
                    "snippets": ["Sunny with a high of 25°C in Tokyo today."]
                }
            ],
            "map": []
        },
        "sources": {
            "https://example.com/weather-tokyo": {
                "title": "Tokyo Weather Today",
                "hostname": "example.com"
            }
        }
    }))
    .into_response()
}
