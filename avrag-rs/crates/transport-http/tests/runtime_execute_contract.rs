use app_bootstrap::AppState;
use app_core::AppConfig;
use async_trait::async_trait;
use avrag_rag_core::{RagRuntime, test_doubles::test_rag_config};
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, MultimodalSearchRequest,
    RetrievalReadPort, ScoredChunk, TextDenseSearchRequest,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateApiKeyRequest, CreateDocumentRequest, CreateWorkspaceRequest};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use contracts::documents::DocumentStatus;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingPlane {
    dense_doc_ids: Arc<Mutex<Vec<Option<Vec<Uuid>>>>>,
}

#[async_trait]
impl RetrievalReadPort for RecordingPlane {
    async fn search_text_dense(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        self.dense_doc_ids.lock().unwrap().push(request.doc_ids.clone());
        Ok(Vec::new())
    }

    async fn search_bm25(&self, _request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        Ok(Bm25SearchOutput {
            chunks: Vec::new(),
            trace: Bm25SearchTrace {
                backend: "stub".into(),
                raw_hit_count: 0,
                hydrated_hit_count: 0,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn post_runtime_execute_requires_auth_context() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "calls": [
                            { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["hello"] } }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn post_runtime_execute_rejects_empty_calls() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-owner-user-id", Uuid::new_v4().to_string())
                .body(Body::from(serde_json::json!({ "calls": [] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("invalid_calls")
    );
}

#[tokio::test]
async fn post_runtime_execute_fails_closed_without_runtime() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-owner-user-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "calls": [
                            { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["hello"] } }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("rag_runtime_not_configured")
    );
}

#[tokio::test]
async fn get_runtime_execute_is_not_allowed() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/runtime/execute")
                .header("x-owner-user-id", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn post_runtime_execute_scopes_to_auth_workspace() {
    let owner_user_id = Uuid::new_v4();
    let state = AppState::new(AppConfig::default());
    state.set_uses_memory_adapters(false);
    let state = state.with_auth(AuthContext::new(
        UserId::from(owner_user_id),
        SubjectKind::User,
    ));

    // Product App surface (T1): workspace ops live on WorkspaceApp, not AppState.
    let ws = state.workspace();
    let notebook = ws
        .create_workspace(CreateWorkspaceRequest {
            name: "runtime-execute-scope".to_string(),
            description: "runtime execute scope contract test".to_string(),
        })
        .await
        .unwrap();
    let upload = ws
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
    ws.put_uploaded_document(&upload.document_id, b"atlas rollback checklist".to_vec())
        .await
        .unwrap();
    ws.transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();
    drop(ws);

    // A workspace-bound API key carries workspace scope through the middleware
    // (the only auth path that sets `workspace_id`).
    let key = state
        .admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "runtime-execute".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();

    let in_scope_doc_uuid = Uuid::parse_str(&upload.document_id).unwrap();
    let out_of_scope_doc_uuid = Uuid::new_v4();

    // Inject a rag_runtime backed by a recording data plane.
    let mut state = state;
    let plane = Arc::new(RecordingPlane::default());
    let runtime = RagRuntime::with_data_plane(test_rag_config(), plane.clone());
    state.test_set_rag_runtime(Arc::new(runtime));

    let app = build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", key.plaintext_key))
                .body(Body::from(
                    serde_json::json!({
                        "calls": [
                            {
                                "tool": "dense_retrieval",
                                "version": "1.0",
                                "args": {
                                    "queries": ["hello"],
                                    "doc_scope": [
                                        out_of_scope_doc_uuid.to_string(),
                                        in_scope_doc_uuid.to_string()
                                    ]
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(payload["results"][0]["status"], "ok");

    // The out-of-workspace doc was narrowed away by the scoped dispatch.
    let captured = plane.dense_doc_ids.lock().unwrap().clone();
    assert_eq!(captured, vec![Some(vec![in_scope_doc_uuid])]);
}
