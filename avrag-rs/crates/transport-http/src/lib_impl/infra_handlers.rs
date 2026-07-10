use app_bootstrap::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::Extension;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::handlers;
use crate::middleware::RequestState;

#[derive(Debug, Deserialize)]
pub(crate) struct SignedUploadQuery {
    expires: u64,
    signature: String,
}

pub(crate) async fn health_handler(State(state): State<AppState>) -> Response {
    let mut components = vec!["api".to_string()];
    if state.postgres_configured() {
        if state.pg_ready().await {
            components.push("postgres:ok".to_string());
        } else {
            telemetry::prometheus::record_dependency_failure("postgres");
            components.push("postgres:degraded".to_string());
        }
    }
    (
        StatusCode::OK,
        Json(json!({"status": "ok", "components": components})),
    )
        .into_response()
}

pub(crate) async fn ready_handler(State(state): State<AppState>) -> Response {
    let mut ready = true;
    let mut details = Vec::new();

    if state.postgres_configured() {
        match state.pg_ready().await {
            true => details.push("postgres:ok"),
            false => {
                telemetry::prometheus::record_dependency_failure("postgres");
                details.push("postgres:fail");
                ready = false;
            }
        }
    }

    if ready {
        (
            StatusCode::OK,
            Json(json!({"ready": true, "checks": details})),
        )
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ready": false, "checks": details})),
        )
            .into_response()
    }
}

pub(crate) async fn metrics_handler() -> Response {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        telemetry::prometheus::encode_metrics(),
    )
        .into_response()
}

pub(crate) async fn docs_handler() -> Response {
    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>Context OS API</title></head>
  <body>
    <h1>Context OS API</h1>
    <p>OpenAPI spec: <a href="/openapi.json">/openapi.json</a></p>
  </body>
</html>"#,
    )
        .into_response()
}

pub(crate) async fn openapi_handler() -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "openapi": "3.1.0",
            "info": {
                "title": "Context OS API",
                "version": "0.1.0"
            },
            "paths": {
                "/health": {},
                "/ready": {},
                "/metrics": {},
                "/api/auth/usage-limit": {},
                "/api/v1/chat": {},
                "/api/v1/mcp": {},
                "/v1/workspaces/{workspace_id}/chat/completions": {},
                "/mcp/workspaces/{workspace_id}": {},
                "/mcp/workspaces/{workspace_id}/tools/call": {},
                "/webhooks/stripe": {}
            }
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Stub handlers (JSON 501)
// ---------------------------------------------------------------------------

pub(crate) async fn dev_upload_handler(
    Path(document_id): Path<String>,
    Extension(RequestState(state)): Extension<RequestState>,
    body: Bytes,
) -> Response {
    let node_env = std::env::var("NODE_ENV").unwrap_or_default();
    let e2e_enabled = std::env::var("E2E_ENABLED").unwrap_or_default();
    if node_env == "production" || e2e_enabled != "true" {
        warn!(
            node_env = %node_env,
            e2e_enabled = %e2e_enabled,
            "dev upload rejected: environment gate failed"
        );
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "dev upload not enabled in this environment" })),
        )
            .into_response();
    }

    let upload_state = match state
        .upload_state_for_authenticated_document(&document_id)
        .await
    {
        Ok((upload_state, _)) => upload_state,
        Err(error) => return handlers::app_error_response(error),
    };

    if let Err(error) = upload_state.workspace()
        .put_uploaded_document(&document_id, body.to_vec())
        .await
    {
        return handlers::app_error_response(error);
    }

    match upload_state.workspace().complete_document_upload(&document_id).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => handlers::app_error_response(error),
    }
}

pub(crate) async fn signed_upload_handler(
    Path(document_id): Path<String>,
    Query(query): Query<SignedUploadQuery>,
    State(state): State<AppState>,
    body: Bytes,
) -> Response {
    let (upload_state, object_path) = match state
        .upload_state_for_system_document(&document_id)
        .await
    {
        Ok(value) => value,
        Err(error) => return handlers::app_error_response(error),
    };

    match object_path {
        Some(object_path) => {
            if let Err(error) = upload_state.verify_upload_signature(
                &document_id,
                &object_path,
                query.expires,
                &query.signature,
            ) {
                return handlers::app_error_response(error);
            }
        }
        None if state.postgres_configured() => {
            return handlers::app_error_response(common::AppError::internal(
                "upload object path is not configured",
            ));
        }
        None => {}
    }

    if body.len() as u64 > state.max_upload_file_size_bytes() {
        return handlers::app_error_response(common::AppError::validation(
            "file_too_large",
            format!(
                "upload body size {} exceeds maximum allowed size of {} bytes",
                body.len(),
                state.max_upload_file_size_bytes()
            ),
        ));
    }

    match upload_state.workspace()
        .put_uploaded_document(&document_id, body.to_vec())
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => handlers::app_error_response(error),
    }
}

pub(crate) async fn billing_webhook_handler(
    Path(provider_str): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let provider = match provider_str.parse::<avrag_billing::BillingProvider>() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let signature = match provider {
        avrag_billing::BillingProvider::Stripe => headers
            .get("stripe-signature")
            .and_then(|value| value.to_str().ok()),
        avrag_billing::BillingProvider::Creem => headers
            .get("creem-signature")
            .and_then(|value| value.to_str().ok()),
        avrag_billing::BillingProvider::Alipay => None,
    };

    let result = state
        .billing_api()
        .handle_webhook(provider, signature, body.as_ref())
        .await;

    if provider == avrag_billing::BillingProvider::Alipay && result.ok {
        return (StatusCode::OK, "success").into_response();
    }

    let status = if result.ok {
        StatusCode::OK
    } else {
        match result.error.as_ref().map(|error| error.code.as_str()) {
            Some("billing_webhook_signature_failed" | "billing_webhook_invalid") => {
                StatusCode::BAD_REQUEST
            }
            Some("billing_unconfigured" | "billing_webhook_unavailable") => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    };

    (status, Json(result)).into_response()
}

pub(crate) async fn openai_chat_completions_handler(
    Path(workspace_id): Path<String>,
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(mut req): Json<contracts::chat::ChatRequest>,
) -> Response {
    req.workspace_id = Some(workspace_id.clone());
    if let Err(error) =
        crate::mcp::expand_external_workspace_rag_scope(&state, &workspace_id, &mut req).await
    {
        return handlers::app_error_response(error);
    }
    handlers::chat_post_handler(Extension(RequestState(state)), headers, Json(req)).await
}

pub(crate) async fn shared_workspace_handler(
    Path(token): Path<String>,
    State(state): State<AppState>,
) -> Response {
    if !state.postgres_configured() {
        telemetry::prometheus::record_dependency_failure("postgres");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "error": "Shared notebook service unavailable",
            })),
        )
            .into_response();
    }

    match state.share().get_shared_workspace(&token).await {
        Ok(Some(payload)) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "data": payload,
            })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(json!({
                "success": false,
                "error": "Invalid or expired share token",
            })),
        )
            .into_response(),
        Err(error) => handlers::app_error_response(error),
    }
}

// ---------------------------------------------------------------------------
// Object-storage webhook handler (S3/MinIO event trigger)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct S3Event {
    #[serde(default)]
    records: Vec<S3EventRecord>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct S3EventRecord {
    event_name: String,
    s3: S3Entity,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct S3Entity {
    bucket: S3Bucket,
    object: S3Object,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct S3Bucket {
    name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct S3Object {
    key: String,
}

pub(crate) async fn object_storage_webhook_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> Response {
    let event: S3Event = match serde_json::from_slice(body.as_ref()) {
        Ok(event) => event,
        Err(error) => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "invalid_event_json",
                &format!("failed to parse S3 event: {error}"),
            );
        }
    };

    let mut processed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for record in event.records {
        if !record.event_name.contains("ObjectCreated") {
            skipped += 1;
            continue;
        }

        let key = record.s3.object.key.replace('+', " ");

        let document_id = match extract_document_id_from_object_path(&key) {
            Some(id) => id,
            None => {
                skipped += 1;
                errors.push(format!("unable to extract document_id from key: {key}"));
                continue;
            }
        };

        let (upload_state, _) = match state
            .upload_state_for_system_document(&document_id)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                failed += 1;
                errors.push(format!("document {document_id}: {error}"));
                continue;
            }
        };

        match upload_state.workspace().complete_document_upload(&document_id).await {
            Ok(_) => {
                processed += 1;
            }
            Err(error) => {
                failed += 1;
                errors.push(format!("document {document_id}: {error}"));
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "processed": processed,
            "failed": failed,
            "skipped": skipped,
            "errors": errors,
        })),
    )
        .into_response()
}

fn extract_document_id_from_object_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    // Expected format: {owner_user_id}/{workspace_id}/{document_id}/{filename}
    if parts.len() >= 3 {
        Some(parts[2].to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
