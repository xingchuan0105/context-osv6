// Tests for the linear chat pipeline (replacement for graphflow_tests.rs).

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use app_admin::AdminContext;
    use app_billing::BillingContext;
    use app_core::{AnalyticsServiceCtx, MemoryState, ObjectStorePort, StorageContext};
    use app_documents::DocumentContext;
    use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
    use avrag_guardrails::GuardPipeline;
    use common::{AppError, now_rfc3339, new_id};
use contracts::chat::{ChatRequest};
use contracts::notebooks::{ChatSession, Notebook};
    use tokio::sync::RwLock;
    use uuid::Uuid;

    use crate::chat::pipeline_steps::dispatch_mode;
    use crate::{ChatContext, LlmContext, OrchestratorContext};

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
            storage: StorageContext::new(
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                Arc::new(RwLock::new(memory)),
                Arc::new(RwLock::new(BTreeMap::new())),
                10 * 1024 * 1024,
                true,
                Arc::new(TestObjectStore),
                "http://localhost".to_string(),
                "/tmp/avrag-test".to_string(),
                3600,
                3600,
            ),
            llm_ctx: LlmContext::new(None, None),
            orchestrator: OrchestratorContext::new(
                None,
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
            notebook_id: Some("notebook-1".to_string()),
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
            notebook_id: "notebook-1".to_string(),
            title: None,
            agent_type: agent_type.to_string(),
            summary: None,
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
    async fn dispatch_rag_with_memory_adapters_uses_memory_chat_compat() {
        let notebook_id = new_id();
        let notebook = Notebook {
            id: notebook_id.clone(),
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
        let request = request_with_mode("rag", vec![notebook_id.clone()]);
        let mut session = session_for("rag");
        session.notebook_id = notebook_id;

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "rag");
        assert_eq!(execution.response.session_id, session.id);
        assert!(!execution.apply_output_guard);
    }
}
