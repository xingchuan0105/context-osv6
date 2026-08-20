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
    use app_core::chat_persistence::{
        AppendChatTurn, ChatCatalogPort, ChatContentPort, ChatPersistencePort, ChatSideEffectPort,
        MessagePort, ProfilePort, SessionPort,
    };
    use app_core::domain_rows::{
        ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, MultimodalChunkRow,
        NotificationCreateParams, UserProfileRow,
    };
    use app_documents::AuditRecord;
    use avrag_guardrails::GuardPipeline;
    use common::{AppError, IndexedChunk, SourceRow, SummaryMetadata, new_id, now_rfc3339};
    use contracts::auth_runtime::{ActorId, AuthContext, SubjectKind, UserId};
    use contracts::chat::{ChatEvent, ChatMessage, ChatRequest};
    use contracts::workspaces::{ChatSession, Workspace};
    use tokio::sync::RwLock;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    use uuid::Uuid;

    use crate::chat::pipeline::{ChatExecution, PipelineLane, execute_pipeline_stream};
    use crate::chat::pipeline_steps::{dispatch_mode, inject_assembled_metadata};
    use crate::{
        CapabilitySet, ChatContext, LlmContext, OrchestratorContext, assemble_mode,
        resolve_capabilities,
    };
    use contracts::chat::{ChatResponse, ModeDebug, TraceInfo};

    use crate::agents::service::UnifiedAgentService;
    use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult};
    use async_trait::async_trait;
    use std::sync::Mutex;

    use crate::profile_update::{ProfileDelta, ProfileDeltaStrategy};

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

    /// ChatPersistencePort wrapper that records spine-stage markers (audit, persist)
    /// into a shared vec, letting the pipeline test lock the real stage order.
    struct RecordingChatPersistence {
        inner: Arc<app_core::MemoryChatPersistence>,
        markers: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingChatPersistence {
        fn new(inner: Arc<app_core::MemoryChatPersistence>, markers: Arc<Mutex<Vec<String>>>) -> Self {
            Self { inner, markers }
        }
    }

    #[async_trait]
    impl SessionPort for RecordingChatPersistence {
        async fn search_sessions(&self, auth: &AuthContext, pattern: &str) -> Result<Vec<ChatSession>, AppError> {
            self.inner.search_sessions(auth, pattern).await
        }
        async fn list_sessions(&self, auth: &AuthContext, workspace_id: Option<Uuid>) -> Result<Vec<ChatSession>, AppError> {
            self.inner.list_sessions(auth, workspace_id).await
        }
        async fn get_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<Option<ChatSession>, AppError> {
            self.inner.get_session(auth, session_id).await
        }
        async fn create_session(&self, auth: &AuthContext, workspace_id: Uuid, title: Option<&str>, agent_type: &str) -> Result<ChatSession, AppError> {
            self.inner.create_session(auth, workspace_id, title, agent_type).await
        }
        async fn update_session(&self, auth: &AuthContext, session_id: Uuid, title: Option<&str>, pinned: Option<bool>) -> Result<Option<ChatSession>, AppError> {
            self.inner.update_session(auth, session_id, title, pinned).await
        }
        async fn delete_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<bool, AppError> {
            self.inner.delete_session(auth, session_id).await
        }
    }

    #[async_trait]
    impl MessagePort for RecordingChatPersistence {
        async fn list_messages(&self, auth: &AuthContext, session_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
            self.inner.list_messages(auth, session_id).await
        }
        async fn get_message(&self, auth: &AuthContext, session_id: Uuid, message_id: i64) -> Result<Option<ChatMessage>, AppError> {
            self.inner.get_message(auth, session_id, message_id).await
        }
        async fn append_chat_turn(&self, auth: &AuthContext, session_id: Uuid, turn: AppendChatTurn<'_>) -> Result<i64, AppError> {
            self.markers.lock().unwrap().push("persist".to_string());
            self.inner.append_chat_turn(auth, session_id, turn).await
        }
        async fn search_conversation_history(
            &self,
            auth: &AuthContext,
            session_id: Uuid,
            query: &str,
            scope: ConversationHistoryScope,
            limit: i64,
            exclude_message_ids: &[i64],
        ) -> Result<Vec<ConversationHistoryHit>, AppError> {
            self.inner.search_conversation_history(auth, session_id, query, scope, limit, exclude_message_ids).await
        }
    }

    #[async_trait]
    impl ChatCatalogPort for RecordingChatPersistence {
        async fn search_workspaces(&self, auth: &AuthContext, pattern: &str) -> Result<Vec<Workspace>, AppError> {
            self.inner.search_workspaces(auth, pattern).await
        }
        async fn search_sources(&self, auth: &AuthContext, pattern: &str) -> Result<Vec<SourceRow>, AppError> {
            self.inner.search_sources(auth, pattern).await
        }
        async fn get_workspace(&self, auth: &AuthContext, workspace_id: Uuid) -> Result<Option<Workspace>, AppError> {
            self.inner.get_workspace(auth, workspace_id).await
        }
    }

    #[async_trait]
    impl ProfilePort for RecordingChatPersistence {
        async fn get_user_profile(&self, auth: &AuthContext, user_id: Uuid) -> Result<Option<UserProfileRow>, AppError> {
            self.inner.get_user_profile(auth, user_id).await
        }
        async fn upsert_user_profile(&self, auth: &AuthContext, profile: &UserProfileRow) -> Result<(), AppError> {
            self.inner.upsert_user_profile(auth, profile).await
        }
    }

    #[async_trait]
    impl ChatContentPort for RecordingChatPersistence {
        async fn get_document_asset_by_id(&self, auth: &AuthContext, asset_id: Uuid) -> Result<Option<DocumentAssetRow>, AppError> {
            self.inner.get_document_asset_by_id(auth, asset_id).await
        }
        async fn get_multimodal_chunk_by_id(&self, auth: &AuthContext, chunk_id: Uuid) -> Result<Option<MultimodalChunkRow>, AppError> {
            self.inner.get_multimodal_chunk_by_id(auth, chunk_id).await
        }
        async fn get_chunk_by_id(&self, auth: &AuthContext, chunk_id: Uuid) -> Result<Option<IndexedChunk>, AppError> {
            self.inner.get_chunk_by_id(auth, chunk_id).await
        }
        async fn get_summary_metadata(&self, auth: &AuthContext, doc_ids: &[Uuid]) -> Result<Vec<SummaryMetadata>, AppError> {
            self.inner.get_summary_metadata(auth, doc_ids).await
        }
    }

    #[async_trait]
    impl ChatSideEffectPort for RecordingChatPersistence {
        async fn create_notification(&self, auth: &AuthContext, params: NotificationCreateParams) -> Result<(), AppError> {
            self.inner.create_notification(auth, params).await
        }
        async fn record_usage_event(&self, auth: &AuthContext, metric_type: &str, quantity: i64, source: &str) -> Result<(), AppError> {
            self.inner.record_usage_event(auth, metric_type, quantity, source).await
        }
        async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError> {
            self.markers.lock().unwrap().push("audit".to_string());
            self.inner.append_audit_record(record).await
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
            turnstile_token: None,
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

    /// Build a ChatContext with the in-memory chat persistence wired into BOTH the
    /// storage chat_persistence slot and the orchestrator chatmemory (ChatMemory),
    /// plus a seeded workspace + session. Returns the context and the persistence
    /// handle (for asserting stored profile rows).
    fn test_chat_context_with_profiles() -> (
        ChatContext,
        Arc<app_core::MemoryChatPersistence>,
        ChatSession,
        String,
    ) {
        let user_id = Uuid::nil();
        let workspace_id = new_id();
        let session_id = new_id();
        let notebook = Workspace {
            id: workspace_id.clone(),
            owner_user_id: test_auth().user_id().to_string(),
            owner_id: user_id.to_string(),
            name: "Test Workspace".to_string(),
            title: "Test Workspace".to_string(),
            description: String::new(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: Default::default(),
            shared: false,
        };
        let now = now_rfc3339();
        let session = ChatSession {
            id: session_id.clone(),
            workspace_id: workspace_id.clone(),
            title: None,
            agent_type: "chat".to_string(),
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut memory = MemoryState::default();
        memory
            .workspaces
            .insert(workspace_id.clone(), notebook.clone());
        memory.sessions.insert(session_id.clone(), session.clone());
        let memory_arc: Arc<RwLock<MemoryState>> = Arc::new(RwLock::new(memory));
        let chatmem: Arc<app_core::MemoryChatPersistence> =
            Arc::new(app_core::MemoryChatPersistence::new(memory_arc.clone()));
        let state = ChatContext {
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
                    chat_persistence: Some(chatmem.clone()),
                },
                memory: MemoryStateHandles {
                    inner: memory_arc.clone(),
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
                Some(Arc::new(avrag_chatmemory::ChatMemory::new(
                    chatmem.clone(),
                    chatmem.clone(),
                ))),
                Arc::new(GuardPipeline::new()),
                None,
            ),
            analytics: AnalyticsServiceCtx::new(None),
            billing: BillingContext::new(None, "shadow".to_string()),
            admin: AdminContext::new(),
            documents: DocumentContext::new(),
        };
        (state, chatmem, session, session_id)
    }

    fn chat_mode_execution(session_id: &str) -> ChatExecution {
        ChatExecution {
            mode: "chat".to_string(),
            input_usage_text: String::new(),
            apply_output_guard: false,
            response: ChatResponse {
                answer: "echo".to_string(),
                answer_blocks: vec![],
                session_id: session_id.to_string(),
                agent_type: "general".to_string(),
                sources: vec![],
                citations: vec![],
                trace: TraceInfo {
                    mode: "chat".to_string(),
                },
                degrade_trace: vec![],
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(BTreeMap::new()),
                }),
                message_id: None,
                guard_report: None,
                tool_results: vec![],
                usage: None,
                agent_operation_guide: None,
            },
            llm_usage: None,
            debug_metadata: None,
            tokens_emitted: false,
            citations_emitted: false,
            assistant_turn_metadata: None,
        }
    }

    #[tokio::test]
    async fn persist_chat_mode_fresh_user_without_llm_writes_no_profile() {
        let (state, chatmem, session, session_id) = test_chat_context_with_profiles();
        let mut request = request_with_mode("chat", vec![]);
        request.session_id = Some(session_id.clone());
        let mut execution = chat_mode_execution(&session_id.to_string());
        let chat_persistence: Arc<dyn app_core::ChatPersistencePort> = chatmem.clone();

        state
            .persist_chat_execution(&request, &session, &mut execution, chat_persistence.as_ref())
            .await
            .unwrap();

        // dream-v2 is the sole writer; with no LLM clients its strategy yields an
        // empty delta, so a fresh user must not get a profile row at all.
        let profile = chatmem
            .get_user_profile(&test_auth(), Uuid::nil())
            .await
            .unwrap();
        assert!(profile.is_none(), "no LLM -> no profile write");
        let general = execution
            .response
            .mode_debug
            .as_ref()
            .and_then(|m| m.general.as_ref())
            .cloned()
            .unwrap_or_default();
        assert!(!general.contains_key("profile_updated"));
    }

    #[tokio::test]
    async fn persist_chat_mode_without_llm_writes_nothing_on_second_run() {
        let (state, chatmem, session, session_id) = test_chat_context_with_profiles();
        let mut request = request_with_mode("chat", vec![]);
        request.session_id = Some(session_id.clone());
        let mut execution = chat_mode_execution(&session_id.to_string());

        state
            .persist_chat_execution(&request, &session, &mut execution, chatmem.as_ref())
            .await
            .unwrap();
        let after_first = chatmem
            .get_user_profile(&test_auth(), Uuid::nil())
            .await
            .unwrap();
        assert!(after_first.is_none(), "no profile written on first run");

        let mut execution = chat_mode_execution(&session_id.to_string());
        state
            .persist_chat_execution(&request, &session, &mut execution, chatmem.as_ref())
            .await
            .unwrap();

        let after_second = chatmem
            .get_user_profile(&test_auth(), Uuid::nil())
            .await
            .unwrap();
        assert!(
            after_second.is_none(),
            "no LLM -> no profile write on second run either"
        );
        let general = execution
            .response
            .mode_debug
            .as_ref()
            .and_then(|m| m.general.as_ref())
            .cloned()
            .unwrap_or_default();
        assert!(!general.contains_key("profile_updated"));
    }

    #[tokio::test]
    async fn persist_chat_mode_old_inferred_at_without_llm_keeps_seeded_profile() {
        let (state, chatmem, session, session_id) = test_chat_context_with_profiles();
        let mut request = request_with_mode("chat", vec![]);
        request.session_id = Some(session_id.clone());
        // Pre-seed an aged profile: the 24h gate opens, but with no LLM clients the
        // dream strategy yields an empty delta -> no write; the seeded row survives.
        let aged_at = chrono::Utc::now() - chrono::Duration::hours(25);
        chatmem
            .upsert_user_profile(
                &test_auth(),
                &app_core::domain_rows::UserProfileRow {
                    user_id: Uuid::nil(),
                    owner_user_id: test_auth().user_id(),
                    expertise_domains: vec![],
                    preferred_answer_style: None,
                    frequently_asked_topics: vec![],
                    custom_preferences: serde_json::json!({}),
                    structured_profile: serde_json::json!({"seeded": true}),
                    inferred_at: aged_at,
                    inference_version: "seeded".to_string(),
                },
            )
            .await
            .unwrap();

        let mut execution = chat_mode_execution(&session_id.to_string());
        state
            .persist_chat_execution(&request, &session, &mut execution, chatmem.as_ref())
            .await
            .unwrap();

        let profile = chatmem
            .get_user_profile(&test_auth(), Uuid::nil())
            .await
            .unwrap()
            .expect("seeded profile present");
        // dream-v2 gate opened but no LLM -> no write; seeded row untouched.
        assert_eq!(profile.inference_version, "seeded");
        assert_eq!(profile.structured_profile, serde_json::json!({"seeded": true}));
        let general = execution
            .response
            .mode_debug
            .as_ref()
            .and_then(|m| m.general.as_ref())
            .cloned()
            .unwrap_or_default();
        assert!(!general.contains_key("profile_updated"));
    }

    struct FakeProfileDeltaStrategy {
        result: ProfileDelta,
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl ProfileDeltaStrategy for FakeProfileDeltaStrategy {
        async fn infer_delta(
            &self,
            _ctx: &ChatContext,
            _recent_turns: &str,
            _existing_profile: &serde_json::Value,
        ) -> ProfileDelta {
            *self.calls.lock().unwrap() += 1;
            self.result.clone()
        }
    }

    async fn wired_strategy_context() -> (
        ChatContext,
        Arc<app_core::MemoryChatPersistence>,
        Arc<Mutex<usize>>,
    ) {
        let (state, chatmem, _session, _session_id) = test_chat_context_with_profiles();
        (state, chatmem, Arc::new(Mutex::new(0)))
    }

    async fn seed_profile_inferred_at(
        chatmem: &app_core::MemoryChatPersistence,
        inferred_at: chrono::DateTime<chrono::Utc>,
    ) {
        chatmem
            .upsert_user_profile(
                &test_auth(),
                &app_core::domain_rows::UserProfileRow {
                    user_id: Uuid::nil(),
                    owner_user_id: test_auth().user_id(),
                    expertise_domains: vec![],
                    preferred_answer_style: None,
                    frequently_asked_topics: vec![],
                    custom_preferences: serde_json::json!({}),
                    structured_profile: serde_json::json!({"base": true}),
                    inferred_at,
                    inference_version: "seeded".to_string(),
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn dream_gate_skips_strategy_within_24h() {
        let (state, chatmem, calls) = wired_strategy_context().await;
        seed_profile_inferred_at(&chatmem, chrono::Utc::now()).await;

        let strategy = FakeProfileDeltaStrategy {
            result: ProfileDelta {
                global_summary: Some("would write".to_string()),
                ..Default::default()
            },
            calls: calls.clone(),
        };
        let updated = state
            .maybe_update_structured_profile(chatmem.as_ref(), "some turns", &strategy)
            .await;
        assert!(!updated, "24h gate must keep the dream layer from firing");
        assert_eq!(*calls.lock().unwrap(), 0, "strategy must not be consulted");
    }

    #[tokio::test]
    async fn dream_fires_after_24h_and_merges_delta() {
        let (state, chatmem, calls) = wired_strategy_context().await;
        seed_profile_inferred_at(&chatmem, chrono::Utc::now() - chrono::Duration::hours(25))
            .await;

        let strategy = FakeProfileDeltaStrategy {
            result: ProfileDelta {
                global_summary: Some("summarized".to_string()),
                ..Default::default()
            },
            calls: calls.clone(),
        };
        let updated = state
            .maybe_update_structured_profile(chatmem.as_ref(), "some turns", &strategy)
            .await;
        assert!(updated, "aged profile must let the dream layer fire");
        assert_eq!(*calls.lock().unwrap(), 1);

        let profile = chatmem
            .get_user_profile(&test_auth(), Uuid::nil())
            .await
            .unwrap()
            .expect("dream-v2 write present");
        assert_eq!(profile.inference_version, "dream-v2");
        assert_eq!(
            profile.structured_profile["global_summary"],
            serde_json::json!("summarized")
        );
    }

    #[tokio::test]
    async fn pipeline_spine_locks_audit_before_persist() {
        let (mut state, chatmem, _session, session_id) = test_chat_context_with_profiles();

        let markers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let recording = Arc::new(RecordingChatPersistence::new(chatmem.clone(), markers.clone()));
        state.storage = StorageContext::from_parts(StorageContextParts {
            infra: state.storage.infra().clone(),
            stores: StorageStores {
                chat_persistence: Some(recording.clone()),
                ..state.storage.stores().clone()
            },
            memory: state.storage.memory().clone(),
            objects: state.storage.objects().clone(),
        });

        let (tx, mut rx) = mpsc::channel::<ChatEvent>(1024);
        let token = CancellationToken::new();
        let mut request = request_with_mode("chat", vec![]);
        request.session_id = Some(session_id.clone());
        request.stream = true;

        let handle = tokio::spawn({
            let request_id = "spine-lock-test".to_string();
            let pipeline_state = state.clone();
            async move {
                execute_pipeline_stream(
                    pipeline_state,
                    request,
                    request_id,
                    tx,
                    token,
                    PipelineLane::Agent,
                )
                .await
            }
        });

        handle.await.unwrap().expect("pipeline must succeed");
        // Events are drained only after the pipeline fully completes, so this
        // snapshot cannot prove persist-before-Done by interleaving. Persist
        // filling `response.message_id` is observable: Done.message_id > 0.
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        let markers = markers.lock().unwrap().clone();
        assert_eq!(
            markers,
            vec!["audit".to_string(), "persist".to_string()],
            "spine order: audit must precede persist (doc omits audit)"
        );
        assert!(matches!(events.first(), Some(ChatEvent::Start { .. })));
        assert!(
            events.iter().any(|e| matches!(
                e,
                ChatEvent::Done {
                    message_id, ..
                } if *message_id > 0
            )),
            "Done must carry the persisted message_id, not STREAM_PLACEHOLDER_MESSAGE_ID"
        );
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, ChatEvent::Error { .. })),
            "no error event expected"
        );
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

        // A2: rag capability enters single ReAct agent (not orchestrator).
        assert_eq!(execution.mode, "rag");
        assert_eq!(execution.response.session_id, session.id);
        assert!(execution.apply_output_guard);
        assert_eq!(execution.response.agent_type, "rag");
        // Mock agent echoes the user query directly (no synthesize handoff pack).
        assert!(
            execution.response.answer.contains("test"),
            "single-agent answer must contain user question: {}",
            execution.response.answer
        );
        let caps = execution
            .assistant_turn_metadata
            .as_ref()
            .and_then(|m| m.get("capabilities"))
            .cloned();
        assert_eq!(caps, Some(serde_json::json!(["rag"])));
        assert!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .is_none(),
            "orchestrator flag must not be set on single-agent path"
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
        assert_eq!(parts.len(), 4); // agent-base + lead-base + knowledge-base + web

        let cfg_val = req
            .metadata
            .get("assembled_mode_config")
            .cloned()
            .expect("assembled_mode_config");
        let cfg: agent_loop::r#loop::config::ModeConfig =
            serde_json::from_value(cfg_val).expect("deserialize ModeConfig");
        assert_eq!(cfg.id, "rag+search");
        assert!(
            cfg.tool_pool.is_empty(),
            "single-agent: no native retrieval tool_pool, got {:?}",
            cfg.tool_pool
        );
        assert!(
            cfg.sdk_primitives.iter().any(|t| t == "web")
                && cfg.sdk_primitives.iter().any(|t| t == "dense"),
            "dual sdk_primitives: {:?}",
            cfg.sdk_primitives
        );
        assert!(!cfg.worker_handoff);
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

        // Single agent: one Rag run with capability manuals (no Answer-phase pack).
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
            parts
                .iter()
                .any(|p| p.contains("capabilities/knowledge-base/contract.md")),
            "workspace capability expected, got {parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("capabilities/web/contract.md")),
            "web capability expected, got {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("product-answer-base.md")),
            "no orchestrator Answer pack on single-agent path: {parts:?}"
        );
        assert_eq!(captured.kind, crate::agents::AgentKind::Rag);
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
        // Search single-agent applies output guard; no doc-scope clarify required.
        assert!(execution.apply_output_guard);
        assert!(!execution.response.answer.is_empty());
        assert!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .is_none(),
            "orchestrator flag must not be set on single-agent search"
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
            !parts.iter().any(|p| p.contains("capabilities/")),
            "capabilities/* must not load for pure chat: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("answer-")),
            "answer-* blocks must not load for pure chat: {parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("system/agent-base.md")),
            "agent-base must load for pure chat: {parts:?}"
        );
        // No evidence finalize for pure chat.
        assert!(execution.response.citations.is_empty());
    }

    #[tokio::test]
    async fn dispatch_phase_loads_capability_manuals_only() {
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
        assert!(
            execution
                .assistant_turn_metadata
                .as_ref()
                .and_then(|m| m.get("orchestrator"))
                .is_none(),
            "single-agent path has no orchestrator flag"
        );
        // Single ReAct agent with agent-base + workspace capability.
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
        assert!(
            parts
                .iter()
                .any(|p| p.contains("capabilities/knowledge-base/contract.md")),
            "workspace capability must load for rag single agent: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("product-answer-base.md")),
            "no Answer-phase pack on single-agent path: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("orchestrator-base")),
            "orchestrator-base must not load: {parts:?}"
        );
        assert_eq!(captured.kind, crate::agents::AgentKind::Rag);
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
}
