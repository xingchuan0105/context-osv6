// Tests for the linear chat pipeline (replacement for graphflow_tests.rs).

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use app_admin::AdminContext;
    use app_billing::BillingContext;
    use app_core::{
        AnalyticsServiceCtx, MemoryState, MemoryStateHandles, ObjectStoreConfig, ObjectStorePort,
        StorageContext, StorageContextParts, StorageInfra, StorageStores,
    };
    use app_documents::DocumentContext;
    use contracts::auth_runtime::{ActorId, AuthContext, OrgId, SubjectKind};
    use avrag_guardrails::GuardPipeline;
    use common::{AppError, new_id, now_rfc3339};
    use contracts::chat::ChatRequest;
    use contracts::notebooks::{ChatSession, Notebook};
    use tokio::sync::RwLock;
    use uuid::Uuid;

    use crate::chat::pipeline_steps::dispatch_mode;
    use crate::{ChatContext, LlmContext, OrchestratorContext};

    use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult};
    use crate::agents::service::UnifiedAgentService;
    use async_trait::async_trait;

    struct PipelineEchoAgent;

    #[async_trait]
    impl Agent for PipelineEchoAgent {
        async fn run(
            &self,
            request: AgentRequest,
            _sink: &dyn agent_loop::events::AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            Ok(AgentRunResult {
                answer: request.query.clone(),
                ..Default::default()
            })
        }
    }

    struct TestObjectStore;

    #[async_trait::async_trait]
    impl ObjectStorePort for TestObjectStore {
        async fn put(&self, _path: &str, _bytes: &[u8]) -> Result<(), AppError> {
            Ok(())
        }

        async fn put_stream(
            &self,
            _path: &str,
            _stream: app_core::ObjectStoreUploadStream,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn get(&self, _path: &str) -> Result<Vec<u8>, AppError> {
            Ok(Vec::new())
        }

        async fn head(
            &self,
            _path: &str,
        ) -> Result<app_core::ObjectStoreMetadata, app_core::ObjectStoreHeadError> {
            Err(app_core::ObjectStoreHeadError::NotFound {
                path: String::new(),
            })
        }

        async fn presigned_get_url(&self, _path: &str, _ttl_secs: u64) -> Result<String, AppError> {
            Ok(String::new())
        }
    }

    fn test_auth() -> AuthContext {
        AuthContext::new(OrgId::from(Uuid::nil()), SubjectKind::User)
            .with_actor_id(ActorId::new(Uuid::nil()))
            .with_request_id("pipeline-test")
    }

    fn test_chat_context(notebook: Option<Notebook>) -> ChatContext {
        let mut memory = MemoryState::default();
        if let Some(notebook) = notebook {
            memory
                .notebooks
                .insert(notebook.id.clone(), notebook.clone());
        }
        ChatContext {
            auth: test_auth(),
            storage: StorageContext::from_parts(StorageContextParts {
                infra: StorageInfra {
                    postgres_health: None,
                    postgres_configured: false,
                    uses_memory_adapters: StorageInfra::memory_adapters_flag(true),
                    max_upload_file_size_bytes: 10 * 1024 * 1024,
                },
                stores: StorageStores {
                    document_store: None,
                    auth_store: None,
                    admin_store: None,
                    billing_quota: None,
                    billing_store: None,
                    share_store: None,
                    chat_persistence: None,
                },
                memory: MemoryStateHandles {
                    inner: Arc::new(RwLock::new(memory)),
                    api_keys: Arc::new(RwLock::new(BTreeMap::new())),
                    api_key_hashes: Arc::new(RwLock::new(BTreeMap::new())),
                },
                objects: ObjectStoreConfig {
                    object_store: Arc::new(TestObjectStore),
                    public_base_url: "http://localhost".to_string(),
                    object_root: "/tmp/avrag-test".to_string(),
                    upload_expire_sec: 3600,
                    download_expire_sec: 3600,
                },
            }),
            llm_ctx: LlmContext::new(None, None),
            orchestrator: OrchestratorContext::new(
                Some(Arc::new(UnifiedAgentService::new(Box::new(
                    PipelineEchoAgent,
                )))),
                None,
                Arc::new(GuardPipeline::new()),
                None,
            ),
            analytics: AnalyticsServiceCtx::new(None),
            billing: BillingContext::new(None, "shadow".to_string()),
            admin: AdminContext::new(),
            documents: DocumentContext::new(),
        }
    }

    fn request_with_mode(agent_type: &str, doc_scope: Vec<String>) -> ChatRequest {
        ChatRequest {
            query: "test".to_string(),
            workspace_id: Some("notebook-1".to_string()),
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope,
            messages: vec![],
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
        }
    }

    fn session_for(agent_type: &str) -> ChatSession {
        let now = now_rfc3339();
        ChatSession {
            id: "session-1".to_string(),
            workspace_id: "notebook-1".to_string(),
            title: None,
            agent_type: agent_type.to_string(),
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn dispatch_rag_without_docscope_returns_clarify_response() {
        let state = test_chat_context(None);
        let request = request_with_mode("rag", vec![]);
        let session = session_for("rag");

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "rag");
        assert!(!execution.apply_output_guard);
        assert!(execution.response.citations.is_empty());
        assert!(execution.response.sources.is_empty());
        assert!(!execution.response.answer.is_empty());
    }

    #[tokio::test]
    async fn dispatch_rag_with_notebook_docscope_runs_rag_pipeline() {
        let workspace_id = new_id();
        let notebook = Notebook {
            id: workspace_id.clone(),
            org_id: test_auth().org_id().to_string(),
            owner_id: Uuid::nil().to_string(),
            name: "Test Notebook".to_string(),
            title: "Test Notebook".to_string(),
            description: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: Default::default(),
            shared: false,
        };
        let state = test_chat_context(Some(notebook.clone()));
        let request = request_with_mode("rag", vec![workspace_id.clone()]);
        let mut session = session_for("rag");
        session.workspace_id = workspace_id;

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "rag");
        assert_eq!(execution.response.session_id, session.id);
        assert!(execution.apply_output_guard);
        assert_eq!(execution.response.answer, "test");
    }
}
