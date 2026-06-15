//! Mock Paddle OCR Jobs HTTP server for Product E2E ingest tests.

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    extract::{Multipart, Path},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// OCR text returned by the mock Paddle Jobs API for image ingest contract tests.
pub const MOCK_PADDLE_IMAGE_OCR_TEXT: &str =
    "Paddle image contract OCR text for product E2E.";

/// JSONL body mimicking Paddle AI Studio layout parsing output (searchable text).
pub(crate) fn mock_paddle_ocr_result_jsonl() -> String {
    format!(
        r#"{{"layoutParsingResults":[{{"markdown":{{"text":"{text}","images":{{}}}}}}]}}"#,
        text = MOCK_PADDLE_IMAGE_OCR_TEXT
    )
}

async fn mock_paddle_submit_job(
    mut multipart: Multipart,
    jobs_submitted: Arc<AtomicUsize>,
    base_url: Arc<String>,
) -> axum::response::Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        let _ = field.bytes().await;
    }
    let job_id = format!("mock-paddle-{}", jobs_submitted.fetch_add(1, Ordering::SeqCst) + 1);
    let json_url = format!("{base_url}/results/{job_id}");
    Json(json!({
        "data": {
            "jobId": job_id,
            "resultUrl": { "jsonUrl": json_url }
        }
    }))
    .into_response()
}

async fn mock_paddle_poll_job(
    Path(job_id): Path<String>,
    base_url: Arc<String>,
) -> axum::response::Response {
    Json(json!({
        "data": {
            "state": "done",
            "resultUrl": {
                "jsonUrl": format!("{base_url}/results/{job_id}")
            }
        }
    }))
    .into_response()
}

async fn mock_paddle_result_json(Path(job_id): Path<String>) -> axum::response::Response {
    let _ = job_id;
    mock_paddle_ocr_result_jsonl().into_response()
}

/// Start a mock Paddle OCR Jobs HTTP server (submit → poll → jsonUrl fetch).
///
/// Returns (base_url, abort_sender, jobs_submitted_counter).
pub async fn start_mock_paddle_ocr_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicUsize>,
) {
    let jobs_submitted = Arc::new(AtomicUsize::new(0));
    let (listener, base_url) = bind_persistent_listener().await;
    let base_url = Arc::new(base_url);
    let submit_base = base_url.clone();
    let poll_base = base_url.clone();
    let jobs_for_submit = jobs_submitted.clone();

    let app = Router::new()
        .route(
            "/jobs",
            post(move |multipart| {
                mock_paddle_submit_job(multipart, jobs_for_submit.clone(), submit_base.clone())
            }),
        )
        .route(
            "/jobs/{job_id}",
            get(move |path| mock_paddle_poll_job(path, poll_base.clone())),
        )
        .route("/results/{job_id}", get(mock_paddle_result_json));

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (
        Arc::try_unwrap(base_url).unwrap_or_else(|arc| (*arc).clone()),
        abort_tx,
        jobs_submitted,
    )
}
