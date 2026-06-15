//! Mock Embedding HTTP server for Product E2E.

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    response::IntoResponse,
    routing::post,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Start a mock Embedding HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, embedding_should_503_flag, call_count).
pub(crate) async fn start_mock_embedding_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicBool>,
    Arc<AtomicUsize>,
) {
    let embedding_should_503 = Arc::new(AtomicBool::new(false));
    let embedding_call_count = Arc::new(AtomicUsize::new(0));
    let flag = embedding_should_503.clone();
    let call_count = embedding_call_count.clone();

    let flag_mm = embedding_should_503.clone();
    let call_count_mm = embedding_call_count.clone();
    let app = Router::new()
        .route(
            "/embeddings",
            post(move |req| mock_embedding_handler(req, flag.clone(), call_count.clone())),
        )
        .fallback(post(move |req| {
            mock_dashscope_multimodal_embedding_handler(req, flag_mm.clone(), call_count_mm.clone())
        }));

    let (listener, base_url) = bind_persistent_listener().await;

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (
        base_url,
        abort_tx,
        embedding_should_503,
        embedding_call_count,
    )
}

async fn mock_dashscope_multimodal_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
    embedding_call_count: Arc<AtomicUsize>,
) -> axum::response::Response {
    embedding_call_count.fetch_add(1, Ordering::SeqCst);

    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "code": "ServiceUnavailable", "message": "embedding service unavailable" })),
        )
            .into_response();
    }

    let dim = req["parameters"]["dimension"]
        .as_u64()
        .or_else(|| req["parameters"]["dimensions"].as_u64())
        .unwrap_or(1024) as usize;
    let fused = req["parameters"]["enable_fusion"].as_bool().unwrap_or(false);
    let contents_len = req["input"]["contents"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(1)
        .max(1);
    let embedding_type = if fused || contents_len > 1 {
        "fusion"
    } else {
        "text"
    };
    // Stable vector so multimodal dense retrieval always matches indexed chunks.
    let embedding: Vec<f32> = (0..dim)
        .map(|j| 0.1_f32 + (j % 10) as f32 * 0.01)
        .collect();

    Json(json!({
        "output": {
            "embeddings": [{
                "index": 0,
                "embedding": embedding,
                "type": embedding_type
            }]
        },
        "usage": {
            "input_tokens": 10,
            "input_tokens_details": {
                "image_tokens": 0,
                "text_tokens": 10
            },
            "output_tokens": 1,
            "total_tokens": 11
        }
    }))
    .into_response()
}

async fn mock_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
    embedding_call_count: Arc<AtomicUsize>,
) -> axum::response::Response {
    embedding_call_count.fetch_add(1, Ordering::SeqCst);

    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "embedding service unavailable" })),
        )
            .into_response();
    }

    let texts = req["input"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
    // All vectors identical so dense retrieval always returns high similarity.
    let vec: Vec<f32> = (0..dim).map(|j| 0.1_f32 + (j % 10) as f32 * 0.01).collect();
    let data: Vec<serde_json::Value> = texts.iter().map(|_| json!({"embedding": vec})).collect();

    Json(json!({ "data": data, "model": "mock-embedding" })).into_response()
}
