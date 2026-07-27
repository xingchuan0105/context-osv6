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
    use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
    use avrag_guardrails::GuardPipeline;
    use common::{AppError, new_id, now_rfc3339};
    use contracts::chat::ChatRequest;
    use contracts::workspaces::{ChatSession, Workspace};
    use tokio::sync::RwLock;
    use uuid::Uuid;

    use crate::chat::pipeline_steps::{dispatch_mode, inject_assembled_metadata};
    use crate::{
        assemble_mode, resolve_capabilities, CapabilitySet, ChatContext, LlmContext,
        OrchestratorContext,
    };

    use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult};
    use crate::agents::service::UnifiedAgentService;
    use async_trait::async_trait;
    use std::sync::Mutex;

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

    /// Captures AgentRequest metadata for assembly-path assertions.
    struct MetadataCaptureAgent {
        last: Arc<Mutex<Option<AgentRequest>>>,
    }

    #[async_trait]
    impl Agent for MetadataCaptureAgent {
        async fn run(
            &self,
            request: AgentRequest,
            _sink: &dyn agent_loop::events::AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            let answer = request.query.clone();
            *self.last.lock().unwrap() = Some(request);
            Ok(AgentRunResult {
                answer,
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
        AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::User)
            .with_actor_id(ActorId::new(Uuid::nil()))
            .with_request_id("pipeline-test")
    }

    fn test_chat_context(notebook: Option<Workspace>) -> ChatContext {
        let mut memory = MemoryState::default();
        if let Some(notebook) = notebook {
            memory
                .workspaces
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
            capabilities: None,
            client_context: None,
        client_ip: None,
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
        let notebook = Workspace {
            id: workspace_id.clone(),
            owner_user_id: test_auth().user_id().to_string(),
            owner_id: Uuid::nil().to_string(),
            name: "Test Workspace".to_string(),
            title: "Test Workspace".to_string(),
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

        // Product path: rag capability always enters orchestrator (worker + chat exit).
        assert_eq!(execution.mode, "rag");
        assert_eq!(execution.response.session_id, session.id);
        assert!(execution.apply_output_guard);
        assert_eq!(execution.response.agent_type, "rag");
        // The mock agent echoes its query; the chat exit's query is the
        // synthesize context, which always ends with the user-question block.
        assert!(
            execution.response.answer.contains("### User question"),
            "answer must come from the chat exit synthesize query: {}",
            execution.response.answer
        );
        let caps = execution
            .assistant_turn_metadata
            .as_ref()
            .and_then(|m| m.get("capabilities"))
            .cloned();
        assert_eq!(caps, Some(serde_json::json!(["rag"])));
        assert_eq!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn inject_assembled_metadata_dual_roundtrips_mode_config() {
        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let assembled = assemble_mode(caps).expect("assemble dual");
        let mut req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "q".into(),
            workspace_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth: test_auth(),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        };
        inject_assembled_metadata(&mut req, caps, &assembled);

        let listed = req
            .metadata
            .get("capabilities")
            .and_then(|v| v.as_array())
            .expect("capabilities array");
        assert_eq!(listed.len(), 2);

        let parts = req
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .expect("system_prompt_parts");
        assert_eq!(parts.len(), 2);

        let cfg_val = req
            .metadata
            .get("assembled_mode_config")
            .cloned()
            .expect("assembled_mode_config");
        let cfg: agent_loop::r#loop::config::ModeConfig =
            serde_json::from_value(cfg_val).expect("deserialize ModeConfig");
        assert_eq!(cfg.id, "rag+search");
        assert!(cfg.tool_pool.iter().any(|t| t == "web_search"));
        assert!(
            !cfg.tool_pool.iter().any(|t| t == "user_context"),
            "capability assemble must not seed chat user_context: {:?}",
            cfg.tool_pool
        );
    }

    #[tokio::test]
    async fn dispatch_dual_capabilities_injects_metadata_and_label() {
        let capture = Arc::new(Mutex::new(None));
        let agent = MetadataCaptureAgent {
            last: capture.clone(),
        };
        let workspace_id = new_id();
        let notebook = Workspace {
            id: workspace_id.clone(),
            owner_user_id: test_auth().user_id().to_string(),
            owner_id: Uuid::nil().to_string(),
            name: "Test Workspace".to_string(),
            title: "Test Workspace".to_string(),
            description: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: Default::default(),
            shared: false,
        };
        let mut state = test_chat_context(Some(notebook));
        state.orchestrator = OrchestratorContext::new(
            Some(Arc::new(UnifiedAgentService::new(Box::new(agent)))),
            None,
            Arc::new(GuardPipeline::new()),
            None,
        );

        let mut request = request_with_mode("chat", vec![workspace_id.clone()]);
        request.capabilities = Some(vec!["rag".into(), "search".into()]);
        let mut session = session_for("chat");
        session.workspace_id = workspace_id;

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "rag+search");
        assert_eq!(execution.response.agent_type, "rag+search");
        let caps_meta = execution
            .assistant_turn_metadata
            .as_ref()
            .and_then(|m| m.get("capabilities"))
            .cloned();
        assert_eq!(caps_meta, Some(serde_json::json!(["rag", "search"])));

        // Last agent invocation is the chat exit (Product Agent Answer phase),
        // not the removed flat assemble path.
        let captured = capture.lock().unwrap().take().expect("agent ran");
        assert!(
            captured.metadata.contains_key("assembled_mode_config"),
            "assembled_mode_config must reach agent"
        );
        assert!(
            captured.metadata.contains_key("system_prompt_parts"),
            "system_prompt_parts must reach agent"
        );
        let parts: Vec<String> = captured
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "Answer phase product-answer-base expected, got {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("chat-base.md")),
            "P1-2: full chat-base must not load in Answer pack, got {parts:?}"
        );
        assert_eq!(captured.kind, crate::agents::AgentKind::Chat);
        assert_eq!(execution.mode, "rag+search");
    }

    #[tokio::test]
    async fn dispatch_search_only_capabilities_skips_doc_scope_clarify() {
        let state = test_chat_context(None);
        let mut request = request_with_mode("chat", vec![]);
        request.capabilities = Some(vec!["search".into()]);
        let session = session_for("chat");

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "search");
        assert_eq!(execution.response.agent_type, "search");
        // Search (orchestrated) applies output guard; no doc-scope clarify required.
        assert!(execution.apply_output_guard);
        assert!(!execution.response.answer.is_empty());
        assert_eq!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn resolve_empty_capabilities_wins_over_rag_agent_type() {
        let caps = resolve_capabilities(Some(&[]), "rag").unwrap();
        assert!(caps.is_pure_chat());
    }

    // -----------------------------------------------------------------------
    // PR-D4: Option D phase matrix + eval bridge + error envelope tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn answer_only_never_loads_orchestrator_prompts() {
        let capture = Arc::new(Mutex::new(None));
        let agent = MetadataCaptureAgent {
            last: capture.clone(),
        };
        let state = test_chat_context(None);
        let state = ChatContext {
            orchestrator: OrchestratorContext::new(
                Some(Arc::new(UnifiedAgentService::new(Box::new(agent)))),
                None,
                Arc::new(GuardPipeline::new()),
                None,
            ),
            ..state
        };
        let request = request_with_mode("chat", vec![]);
        let session = session_for("chat");

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "chat");
        let captured = capture.lock().unwrap().take().expect("agent ran");
        let parts: Vec<String> = captured
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        // Pure chat: no orchestrator-base, no capability-*, no answer-* blocks.
        assert!(
            !parts.iter().any(|p| p.contains("orchestrator-base")),
            "orchestrator-base must not load for pure chat: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("capability-")),
            "capability-* must not load for pure chat: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("answer-")),
            "answer-* blocks must not load for pure chat: {parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("chat-base.md")),
            "chat-base must load for pure chat: {parts:?}"
        );
        // No evidence finalize for pure chat.
        assert!(execution.response.citations.is_empty());
    }

    #[tokio::test]
    async fn dispatch_phase_loads_orchestrator_and_capability_prompts() {
        let capture = Arc::new(Mutex::new(None));
        let agent = MetadataCaptureAgent {
            last: capture.clone(),
        };
        let workspace_id = new_id();
        let notebook = Workspace {
            id: workspace_id.clone(),
            owner_user_id: test_auth().user_id().to_string(),
            owner_id: Uuid::nil().to_string(),
            name: "Test Workspace".to_string(),
            title: "Test Workspace".to_string(),
            description: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: Default::default(),
            shared: false,
        };
        let mut state = test_chat_context(Some(notebook));
        state.orchestrator = OrchestratorContext::new(
            Some(Arc::new(UnifiedAgentService::new(Box::new(agent)))),
            None,
            Arc::new(GuardPipeline::new()),
            None,
        );
        let mut request = request_with_mode("chat", vec![workspace_id.clone()]);
        request.capabilities = Some(vec!["rag".into()]);
        let mut session = session_for("chat");
        session.workspace_id = workspace_id;

        let execution = dispatch_mode(&state, &request, &session, None)
            .await
            .unwrap();

        assert_eq!(execution.mode, "rag");
        assert_eq!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        // Workers run; last agent invocation is the chat exit (Answer phase).
        let captured = capture.lock().unwrap().take().expect("agent ran");
        let parts: Vec<String> = captured
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        // Answer phase: product-answer-base (P1-2: no full chat-base, no orchestrator-base).
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "product-answer-base must load for Answer phase: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("chat-base.md")),
            "P1-2: full chat-base must not load in Answer pack: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("orchestrator-base")),
            "orchestrator-base must NOT load for Answer phase: {parts:?}"
        );
    }

    #[tokio::test]
    async fn eval_bridge_store_becomes_retrieval_tool_results() {
        use crate::orchestrator::Channel;
        use crate::orchestrator::EvidenceStore;
        use crate::orchestrator::finalize_answer_evidence;

        let mut store = EvidenceStore::default();
        let tr = contracts::ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({
                "chunks": [{
                    "chunk_id": "chunk-1",
                    "doc_id": "doc-1",
                    "text": "转型报告提到三年规划",
                    "score": 0.9
                }]
            })),
            trace: None,
        };
        store.insert_from_tool_results(Channel::Rag, &[tr]);

        let mut result = AgentRunResult::default();
        result.answer = "报告提到三年规划[[E1]]".into();
        finalize_answer_evidence(&mut result, &store);

        // E-marker rewritten to product cite.
        assert!(result.answer.contains("[[cite:chunk-1]]"));
        assert!(!result.answer.contains("[[E1]]"));
        // Store bridged into tool_results for eval.
        assert!(
            result
                .tool_results
                .iter()
                .any(|tr| tr.tool == "dense_retrieval"),
            "store must bridge to retrieval tool_results"
        );
        assert_eq!(result.citations.len(), 1);
    }

    /// Function-level bridge shape for web evidence. Call sites gate finalize
    /// by exit mode — direct skips it entirely (brain::finish_answer_direct_mode_skips_evidence).
    #[tokio::test]
    async fn finalize_bridges_web_store_for_eval() {
        use crate::orchestrator::Channel;
        use crate::orchestrator::EvidenceStore;
        use crate::orchestrator::finalize_answer_evidence;

        let mut store = EvidenceStore::default();
        let tr = contracts::ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({
                "results": [{
                    "url": "https://example.com",
                    "title": "Example",
                    "snippet": "best practice"
                }]
            })),
            trace: None,
        };
        store.insert_from_tool_results(Channel::Search, &[tr]);

        let mut result = AgentRunResult::default();
        result.answer = "直接回答，无引用".into();
        finalize_answer_evidence(&mut result, &store);

        // No markers to rewrite, but the store bridges when finalize runs.
        assert!(
            result
                .tool_results
                .iter()
                .any(|tr| tr.tool == "web_search"),
            "finalize must bridge web store to tool_results"
        );
        assert!(result.citations.is_empty());
    }

    /// PR-D4: an internal agent error must surface as an error (fail-fast),
    /// never swallowed or rewritten into a user-facing answer / guide.
    #[tokio::test]
    async fn agent_internal_error_propagates_fail_fast() {
        struct FailingAgent;

        #[async_trait]
        impl Agent for FailingAgent {
            async fn run(
                &self,
                _request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                Err(AppError::internal("boom: llm transport down"))
            }
        }

        let state = test_chat_context(None);
        let state = ChatContext {
            orchestrator: OrchestratorContext::new(
                Some(Arc::new(UnifiedAgentService::new(Box::new(FailingAgent)))),
                None,
                Arc::new(GuardPipeline::new()),
                None,
            ),
            ..state
        };
        let request = request_with_mode("chat", vec![]);
        let session = session_for("chat");

        let err = dispatch_mode(&state, &request, &session, None)
            .await
            .expect_err("internal agent error must propagate, not be answered");
        assert!(
            err.to_string().contains("boom: llm transport down"),
            "original error must not be wrapped away: {err}"
        );
    }

    /// G-09: orchestrated (rag) path also fail-fasts on agent internal error.
    #[tokio::test]
    async fn orchestrated_agent_internal_error_propagates_fail_fast() {
        struct FailingAgent;

        #[async_trait]
        impl Agent for FailingAgent {
            async fn run(
                &self,
                _request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                Err(AppError::internal("boom: orchestrated llm down"))
            }
        }

        let workspace_id = new_id();
        let notebook = Workspace {
            id: workspace_id.clone(),
            owner_user_id: test_auth().user_id().to_string(),
            owner_id: Uuid::nil().to_string(),
            name: "Test Workspace".to_string(),
            title: "Test Workspace".to_string(),
            description: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: Default::default(),
            shared: false,
        };
        let mut state = test_chat_context(Some(notebook));
        state.orchestrator = OrchestratorContext::new(
            Some(Arc::new(UnifiedAgentService::new(Box::new(FailingAgent)))),
            None,
            Arc::new(GuardPipeline::new()),
            None,
        );
        let mut request = request_with_mode("chat", vec![workspace_id.clone()]);
        request.capabilities = Some(vec!["rag".into()]);
        let mut session = session_for("chat");
        session.workspace_id = workspace_id;

        let err = dispatch_mode(&state, &request, &session, None)
            .await
            .expect_err("orchestrated internal error must not become a soft answer");
        let msg = err.to_string();
        assert!(
            msg.contains("boom: orchestrated llm down") || msg.contains("orchestrated llm"),
            "original error must surface: {msg}"
        );
        assert!(
            !msg.contains("agent_operation_guide"),
            "error path must not rewrite into operation guide: {msg}"
        );
    }
}
