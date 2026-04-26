use app::{AppConfig, AppState};
use avrag_auth::{AuthContext, OrgId, SubjectKind};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateDocumentRequest, CreateNotebookRequest, DocumentStatus, ExecutePlanResponse};
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

#[tokio::test]
async fn post_rag_execute_plan_returns_bundle_and_trace() {
    let (app, document_id, org_id) = test_app_with_ready_document().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", org_id.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": [document_id],
                        "items": [
                            { "priority": 1.0, "query": "atlas rollback checklist" }
                        ],
                        "summary_mode": "related",
                        "budget": {
                            "total_candidate_budget": 2,
                            "final_chunk_budget": 1
                        },
                        "trace": {
                            "request_id": "req-rag-execute",
                            "origin": "transport-contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: ExecutePlanResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        payload
            .backend_trace
            .trace
            .as_ref()
            .and_then(|trace| trace.request_id.as_deref()),
        Some("req-rag-execute")
    );
    assert!(payload.coverage.retrieved_chunk_count >= 1);
    assert_eq!(payload.coverage.matched_doc_count, 1);
    assert_eq!(
        payload.backend_trace.retrieval_trace.total_candidate_budget,
        2
    );
    assert_eq!(payload.backend_trace.retrieval_trace.max_final_chunks, 1);
    assert_eq!(
        payload.bundle.chunks[0].doc_id,
        payload.bundle.citations[0].doc_id
    );
    assert!(payload.bundle.chunks[0].parse_run_id.is_none());
    assert!(payload.bundle.chunks[0].score_breakdown.is_empty());
    assert!(payload.bundle.graph_supported_chunks.is_empty());
    assert!(payload.bundle.relation_paths.is_empty());
    assert!(payload.bundle.citations[0].parse_run_id.is_none());
    assert_eq!(payload.coverage.channel_coverage.graph, 0);
    assert!(payload.backend_trace.channel_trace.is_empty());
}

#[tokio::test]
async fn post_rag_execute_plan_requires_auth_context() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "items": [{ "priority": 1.0, "query": "atlas" }]
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
async fn post_rag_execute_plan_rejects_invalid_contract_shapes() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "items": [
                            {
                                "priority": 0.5,
                                "query": "atlas",
                                "bm25_terms": ["rollback"]
                            }
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
        Some("invalid_execute_plan")
    );
}

#[tokio::test]
async fn post_rag_execute_plan_rejects_empty_doc_scope() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": [],
                        "items": [{ "priority": 1.0, "query": "atlas" }]
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
        Some("invalid_execute_plan")
    );
}

#[tokio::test]
async fn post_rag_execute_plan_rejects_legacy_session_fields() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": [Uuid::new_v4().to_string()],
                        "items": [{ "priority": 1.0, "query": "atlas" }],
                        "session_id": "session-1",
                        "history": [],
                        "clarify_needed": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn post_rag_execute_plan_rejects_too_many_items() {
    let app = build_router(AppState::new(AppConfig::default()));
    let items = (0..5)
        .map(|index| {
            serde_json::json!({
                "priority": 0.5,
                "query": format!("query {index}")
            })
        })
        .collect::<Vec<_>>();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "items": items
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
        Some("invalid_execute_plan")
    );
}

#[tokio::test]
async fn post_rag_execute_plan_rejects_invalid_doc_scope_uuid() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": ["not-a-uuid"],
                        "items": [{ "priority": 1.0, "query": "atlas" }]
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
        Some("invalid_doc_scope")
    );
}

#[tokio::test]
async fn post_rag_execute_plan_rejects_non_ready_document() {
    let (app, document_id, org_id) = test_app_with_pending_document().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", org_id.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": [document_id],
                        "items": [{ "priority": 1.0, "query": "atlas" }]
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
        Some("invalid_doc_scope")
    );
}

#[tokio::test]
async fn get_rag_execute_plan_is_not_allowed() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rag/execute-plan")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn mcp_notebook_tool_call_expands_empty_doc_scope_to_ready_documents() {
    let (app, notebook_id, document_id, org_id) = test_app_with_ready_notebook_document().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/notebooks/{notebook_id}/tools/call"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", org_id.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "id": "call-1",
                        "params": {
                            "arguments": {
                                "query": "atlas rollback",
                                "agent_type": "rag"
                            }
                        }
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
    assert_eq!(
        payload
            .pointer("/result/citations/0/doc_id")
            .and_then(|value| value.as_str()),
        Some(document_id.as_str())
    );
}

#[tokio::test]
async fn mcp_notebook_tool_call_rejects_empty_doc_scope_without_ready_documents() {
    let (app, notebook_id, org_id) = test_app_with_empty_notebook().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/mcp/notebooks/{notebook_id}/tools/call"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", org_id.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "id": "call-1",
                        "params": {
                            "arguments": {
                                "query": "atlas rollback",
                                "agent_type": "rag"
                            }
                        }
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
        Some("docscope_required")
    );
}

async fn test_app_with_pending_document() -> (axum::Router, String, Uuid) {
    let state = AppState::new(AppConfig::default());
    let org_id = Uuid::new_v4();
    let scoped = state.with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User));
    let notebook = scoped
        .create_notebook(CreateNotebookRequest {
            name: "rag-contract-pending".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let upload = scoped
        .create_document_upload(
            &notebook.id,
            CreateDocumentRequest {
                filename: "pending.txt".to_string(),
                file_size: 32,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .unwrap();

    (build_router(state), upload.document_id, org_id)
}

async fn test_app_with_empty_notebook() -> (axum::Router, String, Uuid) {
    let state = AppState::new(AppConfig::default());
    let org_id = Uuid::new_v4();
    let notebook = state
        .with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User))
        .create_notebook(CreateNotebookRequest {
            name: "rag-contract-empty".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();

    (build_router(state), notebook.id, org_id)
}

async fn test_app_with_ready_notebook_document() -> (axum::Router, String, String, Uuid) {
    let state = AppState::new(AppConfig::default());
    let org_id = Uuid::new_v4();
    let scoped = state.with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User));
    let notebook = scoped
        .create_notebook(CreateNotebookRequest {
            name: "rag-contract-wrapper".to_string(),
            description: String::new(),
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
        .put_uploaded_document(
            &upload.document_id,
            b"atlas rollback checklist and incident timeline".to_vec(),
        )
        .await
        .unwrap();
    scoped
        .transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();

    (build_router(state), notebook.id, upload.document_id, org_id)
}

async fn test_app_with_ready_document() -> (axum::Router, String, Uuid) {
    let state = AppState::new(AppConfig::default());
    let org_id = Uuid::new_v4();
    let scoped = state.with_auth(AuthContext::new(OrgId::from(org_id), SubjectKind::User));
    let notebook = scoped
        .create_notebook(CreateNotebookRequest {
            name: "rag-contract".to_string(),
            description: String::new(),
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
        .put_uploaded_document(
            &upload.document_id,
            b"atlas rollback checklist and incident timeline".to_vec(),
        )
        .await
        .unwrap();
    scoped
        .transition_document_status(&upload.document_id, DocumentStatus::Completed)
        .await
        .unwrap();

    (build_router(state), upload.document_id, org_id)
}
