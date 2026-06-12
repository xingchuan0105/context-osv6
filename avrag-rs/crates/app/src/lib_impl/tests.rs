#[cfg(test)]
mod tests {
    use crate::lib_impl::*;
    use crate::AppConfig;
    use app_documents::{
        build_docscope_metadata, build_url_source_filename, infer_url_import_mime_type,
        normalize_imported_text,
    };
    use common::{CreateDocumentRequest, CreateNotebookRequest, UpdateDocumentRequest};
use contracts::chat::{ChatMessage};
use contracts::documents::{CreateDocumentUploadResponse, DocumentStatus};
use contracts::notebooks::{Notebook};

    use uuid::Uuid;

    use avrag_storage_pg::PgAppRepository;
    use reqwest::Url;
    use std::sync::Arc;
    use tokio::fs;

    #[test]
    fn build_docscope_metadata_dedupes_known_profile_values() {
        let metadata = vec![
            common::SummaryMetadata {
                doc_id: "doc-1".to_string(),
                filename: "atlas-1.md".to_string(),
                docname: "Atlas One".to_string(),
                language: "zh".to_string(),
                domain: common::Domain::Technology,
                genre: common::Genre::Manual,
                era: common::Era::Contemporary,
                author: None,
                publication_date: None,
            },
            common::SummaryMetadata {
                doc_id: "doc-2".to_string(),
                filename: "atlas-2.md".to_string(),
                docname: "Atlas Two".to_string(),
                language: "zh".to_string(),
                domain: common::Domain::Technology,
                genre: common::Genre::Report,
                era: common::Era::Unknown,
                author: None,
                publication_date: None,
            },
            common::SummaryMetadata {
                doc_id: "doc-3".to_string(),
                filename: "atlas-3.md".to_string(),
                docname: "Atlas Three".to_string(),
                language: "en".to_string(),
                domain: common::Domain::Unknown,
                genre: common::Genre::Unknown,
                era: common::Era::Modern,
                author: None,
                publication_date: None,
            },
        ];

        let result = build_docscope_metadata(metadata);

        assert_eq!(result.documents.len(), 3);
        assert_eq!(
            result.profile.languages,
            vec!["en".to_string(), "zh".to_string()]
        );
        assert_eq!(result.profile.domains, vec![common::Domain::Technology]);
        assert_eq!(
            result.profile.genres,
            vec![common::Genre::Report, common::Genre::Manual]
        );
        assert_eq!(
            result.profile.eras,
            vec![common::Era::Modern, common::Era::Contemporary]
        );
    }

    #[test]
    fn app_config_uses_milvus_data_plane() {
        let config = AppConfig::default();

        assert_eq!(config.milvus.url, "http://127.0.0.1:19530");
        assert_eq!(config.milvus.collection_prefix, "avrag");
    }

    #[test]
    fn app_config_defaults_tongyi_embedding_vision_plus_to_multimodal_schema_dimension() {
        let config = AppConfig::default();

        assert_eq!(
            config.mm_embedding.model,
            "tongyi-embedding-vision-plus-2026-03-06"
        );
        assert_eq!(config.mm_embedding.dimensions, Some(1024));
        assert_eq!(config.milvus.multimodal_vector_dim, 1024);
    }

    #[test]
    fn app_config_defaults_agent_llm_to_deepseek_v4_pro() {
        let config = AppConfig::default();

        assert_eq!(config.agent_llm.base_url, "https://api.deepseek.com");
        assert_eq!(config.agent_llm.model, "deepseek-v4-pro");
        assert_eq!(config.agent_llm.enable_thinking, Some(true));
    }

    #[test]
    fn app_config_defaults_memory_llm_to_deepseek_v4_flash() {
        let config = AppConfig::default();

        assert_eq!(config.memory_llm.base_url, "https://api.deepseek.com");
        assert_eq!(config.memory_llm.model, "deepseek-v4-flash");
        assert_eq!(config.memory_llm.enable_thinking, Some(false));
    }

    #[test]
    fn app_config_defaults_ingestion_llm_to_gemini_flash_lite() {
        let config = AppConfig::default();

        assert_eq!(config.ingestion_llm.base_url, "https://www.dmxapi.cn/v1");
        assert_eq!(config.ingestion_llm.model, "gemini-3.1-flash-lite-preview");
    }

    #[test]
    fn build_rag_session_context_drops_blank_summary_and_empty_payload() {
        assert!(AppState::build_rag_session_context(Vec::new(), Some("   ".to_string())).is_none());

        let context = AppState::build_rag_session_context(
            vec![ChatMessage {
                id: 1,
                session_id: "s1".to_string(),
                role: "user".to_string(),
                content: "hello".to_string(),
                answer_blocks: Vec::new(),
                agent_id: None,
                agent_name: None,
                agent_icon: None,
                citations: Vec::new(),
                tool_results: Vec::new(),
                turn_metadata: None,
                resolved_query: None,
                created_at: "2026-03-25T00:00:00Z".to_string(),
            }],
            Some("  carry this forward  ".to_string()),
        )
        .unwrap();

        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.summary.as_deref(), Some("carry this forward"));
    }

    #[test]
    fn infer_url_import_mime_type_prefers_html_when_body_looks_like_html() {
        assert_eq!(
            infer_url_import_mime_type(
                "text/plain",
                br#"<!doctype html><html><body>Hello</body></html>"#
            ),
            "text/html"
        );
    }

    #[test]
    fn build_url_source_filename_uses_title_and_extension() {
        let url = Url::parse("https://example.com/reports/q1").unwrap();
        assert_eq!(
            build_url_source_filename(&url, "text/html", Some("Quarterly / Report")),
            "Quarterly _ Report.html"
        );
    }

    #[test]
    fn normalize_imported_text_collapses_blank_lines() {
        assert_eq!(
            normalize_imported_text("  First line \n\n\n Second line  \n"),
            "First line\nSecond line"
        );
    }

    #[test]
    fn general_profile_custom_preferences_preserves_agent_memory() {
        let mut agent_memory = contracts::preferences::AgentPreferenceMemory::default();
        agent_memory.active.push(contracts::preferences::AgentPreference {
            id: "pref-1".to_string(),
            text: "Use concise answers".to_string(),
            category: "interaction".to_string(),
            scope: "global".to_string(),
            confidence: "explicit".to_string(),
            source: "test".to_string(),
            updated_at: "2026-04-26T00:00:00Z".to_string(),
        });

        let merged = merge_general_profile_custom_preferences(
            serde_json::json!({ "theme": "dark" }),
            agent_memory,
            "hello",
            "hello refined",
        );

        assert_eq!(
            merged.get("theme").and_then(|value| value.as_str()),
            Some("dark")
        );
        assert_eq!(
            merged
                .pointer("/agent_memory/active/0/text")
                .and_then(|value| value.as_str()),
            Some("Use concise answers")
        );
        assert_eq!(
            merged
                .get("last_general_query")
                .and_then(|value| value.as_str()),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn execute_plan_fails_closed_without_rag_runtime() {
        let state = AppState::new(AppConfig::default());
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "budget".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();

        let mut doc_scope = Vec::new();
        for name in ["one.txt", "two.txt"] {
            let upload = state
                .create_document_upload(
                    &notebook.id,
                    CreateDocumentRequest {
                        filename: name.to_string(),
                        file_size: 32,
                        mime_type: "text/plain".to_string(),
                    },
                )
                .await
                .unwrap();
            state
                .put_uploaded_document(&upload.document_id, b"atlas rollback checklist".to_vec())
                .await
                .unwrap();
            state
                .transition_document_status(&upload.document_id, DocumentStatus::Completed)
                .await
                .unwrap();
            doc_scope.push(upload.document_id);
        }

        let error = state
            .execute_rag_execute_plan(common::ExecutePlanRequest {
                plan_version: "rag-execute-v1".to_string(),
                doc_scope,
                items: vec![common::ExecutePlanItem {
                    priority: 1.0,
                    query: Some("atlas".to_string()),
                    bm25_terms: None,
                }],
                summary_mode: common::ExecutePlanSummaryMode::All,
                budget: Some(common::ExecutePlanBudget {
                    total_candidate_budget: Some(4),
                    final_chunk_budget: Some(1),
                    graph_hop_limit: None,
                    graph_fan_out_limit: None,
                }),
                channel_budget: None,
                query_entities: Vec::new(),
                graph_hints: Vec::new(),
                placeholder_triplets: Vec::new(),
                trace: None,
            })
            .await
            .unwrap_err();

        assert_eq!(error.code(), "rag_runtime_not_configured");
    }

    #[tokio::test]
    async fn memory_delete_document_soft_deletes_and_hides_document() {
        let state = AppState::new(AppConfig::default());
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "soft delete".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let upload = state
            .create_document_upload(
                &notebook.id,
                CreateDocumentRequest {
                    filename: "delete-me.txt".to_string(),
                    file_size: 11,
                    mime_type: "text/plain".to_string(),
                },
            )
            .await
            .unwrap();
        state
            .put_uploaded_document(&upload.document_id, b"hello world".to_vec())
            .await
            .unwrap();

        let response = state.delete_document(&upload.document_id).await.unwrap();

        assert_eq!(response.status, "deleting");
        assert!(
            state
                .list_documents(Some(&notebook.id), Some(&upload.document_id))
                .await
                .is_empty()
        );
        assert_eq!(
            state
                .get_document_content(&upload.document_id)
                .await
                .unwrap_err()
                .code(),
            "document_not_found"
        );
    }

    #[tokio::test]
    async fn memory_update_document_rejects_deletion_workflow_statuses() {
        let state = AppState::new(AppConfig::default());
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "status guard".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let upload = state
            .create_document_upload(
                &notebook.id,
                CreateDocumentRequest {
                    filename: "status-guard.txt".to_string(),
                    file_size: 11,
                    mime_type: "text/plain".to_string(),
                },
            )
            .await
            .unwrap();

        let error = state
            .update_document(
                &upload.document_id,
                UpdateDocumentRequest {
                    filename: None,
                    notebook_id: None,
                    status: Some(DocumentStatus::Deleting),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(error.code(), "unsupported_document_status_update");
        assert_eq!(
            state
                .transition_document_status(&upload.document_id, DocumentStatus::Deleted)
                .await
                .unwrap_err()
                .code(),
            "unsupported_document_status_transition"
        );
        assert_eq!(
            state
                .list_documents(Some(&notebook.id), Some(&upload.document_id))
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn explicit_agent_preference_is_stored_without_answer_fact_extraction() {
        let mut config = AppConfig::default();
        config.user_id = "00000000-0000-0000-0000-000000000002".to_string();
        let state = AppState::new(config);

        state
            .remember_explicit_agent_preference("remember that I prefer concise answers")
            .await
            .unwrap();
        state
            .remember_explicit_agent_preference("This answer contains a factual claim.")
            .await
            .unwrap();

        let preferences = state.current_user_preferences().await.unwrap();
        assert_eq!(preferences.agent_memory.active.len(), 1);
        assert_eq!(
            preferences.agent_memory.active[0].text,
            "I prefer concise answers"
        );
    }

    struct ScriptedAgent;

    #[async_trait::async_trait]
    impl crate::agents::runtime::Agent for ScriptedAgent {
        async fn run(
            &self,
            _request: crate::agents::runtime::AgentRequest,
            sink: &dyn crate::agents::events::AgentEventSink,
        ) -> Result<crate::agents::runtime::AgentRunResult, common::AppError> {
            sink.emit(crate::agents::events::AgentEvent::Activity {
                stage: "chat".to_string(),
                message: "Scripted chat".to_string(),
            })
            .await;
            sink.emit(crate::agents::events::AgentEvent::MessageDelta {
                text: "agent ".to_string(),
            })
            .await;
            sink.emit(crate::agents::events::AgentEvent::MessageDelta {
                text: "answer".to_string(),
            })
            .await;
            sink.emit(crate::agents::events::AgentEvent::Done {
                final_message: Some("agent answer".to_string()),
                usage: None,
            })
            .await;

            Ok(crate::agents::runtime::AgentRunResult {
                answer: "agent answer".to_string(),
                usage: Some(crate::agents::runtime::AgentRunUsage {
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

    struct BufferedOnlyAgent;

    #[async_trait::async_trait]
    impl crate::agents::runtime::Agent for BufferedOnlyAgent {
        async fn run(
            &self,
            _request: crate::agents::runtime::AgentRequest,
            sink: &dyn crate::agents::events::AgentEventSink,
        ) -> Result<crate::agents::runtime::AgentRunResult, common::AppError> {
            sink.emit(crate::agents::events::AgentEvent::Activity {
                stage: "chat".to_string(),
                message: "Buffered chat".to_string(),
            })
            .await;

            Ok(crate::agents::runtime::AgentRunResult {
                answer: "buffered answer".to_string(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn chat_stream_routes_chat_through_unified_agent_service() {
        let mut config = AppConfig::default();
        config.agent_llm.api_key = "test-key".to_string();
        let mut state = AppState::new(config);
        state.set_agent_service(crate::agents::service::UnifiedAgentService::new(Box::new(
            ScriptedAgent,
        )));
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "chat".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        state
            .execute_chat_stream(
                contracts::chat::ChatRequest {
                    query: "hello".to_string(),
                    notebook_id: Some(notebook.id),
                    session_id: None,
                    agent_type: "chat".to_string(),
                    source_type: None,
                    source_token: None,
                    doc_scope: Vec::new(),
                    messages: Vec::new(),
                    stream: true,
                    debug: false,
                    language: None,
                    format_hint: None,
                },
                "req-agent-chat".to_string(),
                tx,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(events.iter().any(|event| matches!(
            event,
            contracts::chat::ChatEvent::AnswerStart { agent_type, .. } if agent_type == "chat"
        )));
        let streamed_answer = events
            .iter()
            .filter_map(|event| match event {
                contracts::chat::ChatEvent::Token { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(streamed_answer, "agent answer");
        let done_events = events
            .iter()
            .filter(|event| matches!(event, contracts::chat::ChatEvent::Done { .. }))
            .count();
        assert_eq!(done_events, 1);
        assert!(
            matches!(events.last(), Some(contracts::chat::ChatEvent::Done { payload, .. })
            if payload.get("agent_type").and_then(|value| value.as_str()) == Some("chat")
                && payload.get("answer").and_then(|value| value.as_str()) == Some("agent answer"))
        );
    }

    #[tokio::test]
    async fn chat_stream_emits_buffered_agent_answer_when_no_delta_arrives() {
        let mut config = AppConfig::default();
        config.agent_llm.api_key = "test-key".to_string();
        let mut state = AppState::new(config);
        state.set_agent_service(crate::agents::service::UnifiedAgentService::new(Box::new(
            BufferedOnlyAgent,
        )));
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "chat".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        state
            .execute_chat_stream(
                contracts::chat::ChatRequest {
                    query: "hello".to_string(),
                    notebook_id: Some(notebook.id),
                    session_id: None,
                    agent_type: "chat".to_string(),
                    source_type: None,
                    source_token: None,
                    doc_scope: Vec::new(),
                    messages: Vec::new(),
                    stream: true,
                    debug: false,
                    language: None,
                    format_hint: None,
                },
                "req-agent-buffered-chat".to_string(),
                tx,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(events.iter().any(|event| matches!(
            event,
            contracts::chat::ChatEvent::AnswerStart { agent_type, .. } if agent_type == "chat"
        )));
        let streamed_answer = events
            .iter()
            .filter_map(|event| match event {
                contracts::chat::ChatEvent::Token { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(streamed_answer, "buffered answer");
        let done_events = events
            .iter()
            .filter(|event| matches!(event, contracts::chat::ChatEvent::Done { .. }))
            .count();
        assert_eq!(done_events, 1);
    }

    #[tokio::test]
    async fn search_stream_emits_buffered_agent_answer_when_no_delta_arrives() {
        let mut state = AppState::new(AppConfig::default());
        state.set_agent_service(crate::agents::service::UnifiedAgentService::new(Box::new(
            BufferedOnlyAgent,
        )));
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: "search".to_string(),
                description: String::new(),
            })
            .await
            .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        state
            .execute_chat_stream(
                contracts::chat::ChatRequest {
                    query: "hello".to_string(),
                    notebook_id: Some(notebook.id),
                    session_id: None,
                    agent_type: "search".to_string(),
                    source_type: None,
                    source_token: None,
                    doc_scope: Vec::new(),
                    messages: Vec::new(),
                    stream: true,
                    debug: false,
                    language: None,
                    format_hint: None,
                },
                "req-agent-buffered-search".to_string(),
                tx,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(events.iter().any(|event| matches!(
            event,
            contracts::chat::ChatEvent::AnswerStart { agent_type, .. } if agent_type == "search"
        )));
        let streamed_answer = events
            .iter()
            .filter_map(|event| match event {
                contracts::chat::ChatEvent::Token { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert_eq!(streamed_answer, "buffered answer");
        let done_events = events
            .iter()
            .filter(|event| matches!(event, contracts::chat::ChatEvent::Done { .. }))
            .count();
        assert_eq!(done_events, 1);
    }

    async fn upload_validation_pg_state() -> Option<(AppState, std::path::PathBuf)> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        let object_root = std::env::temp_dir().join(format!(
            "avrag-app-upload-validation-test-{}",
            Uuid::new_v4()
        ));
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let mut config = AppConfig::default();
        config.org_id = Uuid::new_v4().to_string();
        config.user_id = Uuid::new_v4().to_string();
        config.object_root = object_root.to_string_lossy().to_string();

        let mut state = AppState::new(config);
        let old = state.test_storage().clone();
        let pg = Arc::new(repo);
        let document_store = Some(
            Arc::new(app_bootstrap::test_support::PgDocumentStoreAdapter::new(pg.clone()))
                as Arc<dyn app_core::DocumentStorePort>,
        );
        let admin_store = Some(
            Arc::new(app_bootstrap::test_support::PgAdminStoreAdapter::new(pg.clone()))
                as Arc<dyn app_core::AdminStorePort>,
        );
        let chat_persistence = Some(
            Arc::new(app_bootstrap::test_support::PgChatPersistenceAdapter::new(pg.clone()))
                as Arc<dyn app_core::ChatPersistencePort>,
        );
        let postgres_health = Some(
            Arc::new(app_bootstrap::test_support::PgHealthAdapter::new(pg.clone()))
                as Arc<dyn app_core::PostgresHealthPort>,
        );
        let inner = old.inner().clone();
        let api_keys = old.api_keys().clone();
        let max_upload = old.max_upload_file_size_bytes();
        let object_store = old.object_store().clone();
        let public_base_url = old.public_base_url().to_string();
        let object_root_path = old.object_root_path().to_string_lossy().to_string();
        let upload_expire = old.upload_expire_sec();
        let download_expire = old.download_expire_sec();
        state.test_set_postgres(pg.clone());
        state.test_replace_storage(crate::storage_context::StorageContext::new(
            postgres_health,
            true,
            document_store,
            None,
            admin_store,
            None,
            chat_persistence,
            inner,
            api_keys,
            max_upload,
            false,
            object_store,
            public_base_url,
            object_root_path,
            upload_expire,
            download_expire,
        ));
        Some((state, object_root))
    }

    async fn create_upload(
        state: &AppState,
        filename: &str,
        file_size: u64,
    ) -> (Notebook, CreateDocumentUploadResponse) {
        let notebook = state
            .create_notebook(CreateNotebookRequest {
                name: format!("upload validation {filename}"),
                description: String::new(),
            })
            .await
            .unwrap();
        let upload = state
            .create_document_upload(
                &notebook.id,
                CreateDocumentRequest {
                    filename: filename.to_string(),
                    file_size,
                    mime_type: "text/plain".to_string(),
                },
            )
            .await
            .unwrap();
        (notebook, upload)
    }

    async fn document_status(
        state: &AppState,
        notebook_id: &str,
        document_id: &str,
    ) -> DocumentStatus {
        state
            .list_documents(Some(notebook_id), Some(document_id))
            .await
            .into_iter()
            .next()
            .unwrap()
            .status
    }

    async fn ingestion_task_count(state: &AppState, document_id: &str) -> i64 {
        let document_uuid = Uuid::parse_str(document_id).unwrap();
        state
            .postgres_repo()
            .as_ref()
            .unwrap()
            .count_ingestion_tasks_for_document(state.auth(), document_uuid)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn complete_upload_missing_object_marks_invalid_without_task_when_database_available() {
        let Some((state, object_root)) = upload_validation_pg_state().await else {
            return;
        };
        let (notebook, upload) = create_upload(&state, "missing-object.txt", 11).await;

        let error = state
            .complete_document_upload(&upload.document_id)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upload_validation_failed");
        assert_eq!(
            document_status(&state, &notebook.id, &upload.document_id).await,
            DocumentStatus::UploadInvalid
        );
        assert_eq!(ingestion_task_count(&state, &upload.document_id).await, 0);
        let _ = fs::remove_dir_all(object_root).await;
    }

    #[tokio::test]
    async fn complete_upload_size_mismatch_marks_invalid_without_task_when_database_available() {
        let Some((state, object_root)) = upload_validation_pg_state().await else {
            return;
        };
        let body = b"hello world".to_vec();
        let (notebook, upload) = create_upload(&state, "size-mismatch.txt", 12).await;
        state
            .put_uploaded_document(&upload.document_id, body)
            .await
            .unwrap();

        let error = state
            .complete_document_upload(&upload.document_id)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upload_validation_failed");
        assert_eq!(
            document_status(&state, &notebook.id, &upload.document_id).await,
            DocumentStatus::UploadInvalid
        );
        assert_eq!(ingestion_task_count(&state, &upload.document_id).await, 0);
        let _ = fs::remove_dir_all(object_root).await;
    }

    #[tokio::test]
    async fn complete_upload_matching_size_records_validation_when_database_available() {
        let Some((state, object_root)) = upload_validation_pg_state().await else {
            return;
        };
        let body = b"hello world".to_vec();
        let (notebook, upload) = create_upload(&state, "valid-upload.txt", body.len() as u64).await;
        state
            .put_uploaded_document(&upload.document_id, body)
            .await
            .unwrap();

        let response = state
            .complete_document_upload(&upload.document_id)
            .await
            .unwrap();

        assert_eq!(response.status, "queued");
        assert_eq!(
            document_status(&state, &notebook.id, &upload.document_id).await,
            DocumentStatus::Queued
        );
        assert_eq!(ingestion_task_count(&state, &upload.document_id).await, 1);

        let validation = state
            .postgres_repo()
            .as_ref()
            .unwrap()
            .get_document_upload_validation(
                state.auth(),
                Uuid::parse_str(&upload.document_id).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(validation.upload_size_bytes, Some(11));
        assert_eq!(
            validation.upload_sha256.as_deref(),
            Some("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
        assert!(validation.upload_validated_at.is_some());
        assert_eq!(validation.upload_validation_error, None);
        let _ = fs::remove_dir_all(object_root).await;
    }
}
