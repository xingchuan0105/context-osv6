//! UnifiedAgent — single agent implementation that routes between
//! Chat / RAG / Search modes via `AgentRequest.kind`.
//!
//! v6 (ADR-0006): Chat / RAG / Search route through `agent_loop::ReActLoop`.
//! Differences between modes are expressed through YAML `ModeConfig` files
//! (`modes/chat.yaml`, `modes/rag.yaml`, `modes/search.yaml`) rather than
//! independent Strategy state machines. Tool execute stays in `agent_tools`.
//!
//! # Write mode (intentional split)
//!
//! **Write is not handled here.** Pipeline dispatch routes
//! `AgentKind::Write` to [`crate::writer::run_write_mode`] in
//! `chat::pipeline_steps::dispatch_mode` before constructing an
//! `AgentRequest`. Write needs a full `ChatContext` (session persistence,
//! draft materialization, refine loop) that the ReAct `UnifiedAgent`
//! surface does not own. Treat "Unified" as the ReAct family of modes;
//! Write is a sibling product mode with its own service boundary.
//!
//! Static strategy metadata lives in `agent_tools::capability::schemas` for API
//! discovery; execution no longer uses the removed v5 strategy state machines.

use agent_loop::audit;
use agent_loop::events::{AgentEvent, AgentEventSink};

use agent_loop::runtime::{Agent, AgentRequest, AgentRunResult};

use app_core::{ChatPersistencePort, ProviderSecretPurpose, ProviderSecretStorePort};
use avrag_llm::{ApiStyle, LlmClient, ModelProviderConfig, TenantContext, UsageObserver};
use avrag_search::SearchProvider;
use common::AppError;
use std::sync::Arc;
use uuid::Uuid;

/// Unified agent that dispatches to Chat / RAG / Search based on `request.kind`.
pub struct UnifiedAgent {
    llm_client: Option<LlmClient>,
    chat_llm_client: Option<LlmClient>,
    search_llm_client: Option<LlmClient>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    usage_observer: Option<Arc<dyn UsageObserver>>,
    /// Cloud BYOK secrets (ADR-0010). When set, chat may resolve user keys.
    provider_secrets: Option<Arc<dyn ProviderSecretStorePort>>,
}

impl UnifiedAgent {
    pub fn new(
        llm_client: Option<LlmClient>,
        chat_llm_client: Option<LlmClient>,
        search_llm_client: Option<LlmClient>,
    ) -> Self {
        Self {
            llm_client,
            chat_llm_client,
            search_llm_client,
            rag_runtime: None,
            search_executor: None,
            chat_persistence: None,
            usage_observer: None,
            provider_secrets: None,
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(mut self, executor: Option<Arc<dyn SearchProvider>>) -> Self {
        self.search_executor = executor;
        self
    }

    pub fn with_usage_observer(mut self, observer: Arc<dyn UsageObserver>) -> Self {
        self.usage_observer = Some(observer);
        self
    }

    pub fn with_provider_secrets(
        mut self,
        secrets: Option<Arc<dyn ProviderSecretStorePort>>,
    ) -> Self {
        self.provider_secrets = secrets;
        self
    }

    /// Resolve cloud BYOK LLM secret for this request.
    async fn resolve_byok_llm(
        &self,
        request: &AgentRequest,
    ) -> Option<app_core::ResolvedProviderSecret> {
        let secrets = self.provider_secrets.as_ref()?;
        let owner = request.auth.user_id().into_uuid();
        let workspace = request.auth.workspace_id();
        match secrets
            .resolve(owner, workspace, ProviderSecretPurpose::Llm)
            .await
        {
            Ok(Some(secret)) => Some(secret),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, owner = %owner, "BYOK resolve failed; using platform key");
                None
            }
        }
    }

    fn bind_byok_client(
        client: Option<LlmClient>,
        byok: Option<&app_core::ResolvedProviderSecret>,
    ) -> Option<LlmClient> {
        match (client, byok) {
            (Some(c), Some(secret)) => Some(c.with_user_credentials(
                secret.api_key.clone(),
                secret.base_url.clone(),
                secret.model_hint.clone(),
            )),
            // G1: no platform key — construct a single-route client from the user's
            // resolved BYOK secret instead of dropping it.
            (None, Some(secret)) => llm_client_from_secret(secret),
            (c, None) => c,
        }
    }
}

/// Build a single-route `LlmClient` from a resolved BYOK secret (ADR-0010 G1).
///
/// BYOK secrets are OpenAI-compatible single-route endpoints; there is no
/// multi-provider pool and no native-dialect routing. Returns `None` when the
/// secret is missing `base_url` or `model_hint`, so callers keep the platform
/// path unchanged (fail-open to platform config, matching the existing overlay).
fn llm_client_from_secret(secret: &app_core::ResolvedProviderSecret) -> Option<LlmClient> {
    const BYOK_DEFAULT_TIMEOUT_MS: u64 = 120_000;

    let base_url = secret.base_url.as_deref()?.trim();
    let model = secret.model_hint.as_deref()?.trim();
    if base_url.is_empty() || model.is_empty() || secret.api_key.is_empty() {
        return None;
    }

    Some(LlmClient::new(ModelProviderConfig {
        base_url: base_url.to_string(),
        api_key: secret.api_key.clone(),
        model: model.to_string(),
        timeout_ms: BYOK_DEFAULT_TIMEOUT_MS,
        api_style: Some(ApiStyle::OpenAi),
        dimensions: None,
        enable_thinking: None,
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    }))
}

#[async_trait::async_trait]
impl Agent for UnifiedAgent {
    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let trace_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| "unified-agent".to_string());

        // Emit observable routing decision (user explicitly selected mode).
        let mode_id = request.kind.as_canonical_str().to_string();
        let _ = sink
            .emit(AgentEvent::RoutingDecision {
                mode_id: mode_id.clone(),
                matched_rule: format!("user-{}", mode_id),
                confidence: 1.0,
                explanation: format!("user explicitly selected {:?} mode", request.kind),
            })
            .await;

        // Emit audit record for routing decision.
        let owner_user_id = request.auth.user_id().to_string();
        let actor_id_owned = request.auth.actor_id().map(|id| id.into_uuid().to_string());
        let audit_record = audit::routing_decision_record(
            &owner_user_id,
            actor_id_owned.as_deref(),
            &trace_id,
            &mode_id,
            "user_explicit",
            1.0,
            &format!("user explicitly selected {:?} mode", request.kind),
        );
        let _ = sink
            .emit(AgentEvent::Audit {
                record: audit_record,
            })
            .await;

        let byok = self.resolve_byok_llm(&request).await;
        let mut tenant = TenantContext {
            owner_user_id: request.auth.user_id().into_uuid(),
            user_id: request
                .auth
                .actor_id()
                .map(|id| id.into_uuid())
                .unwrap_or_else(Uuid::nil),
            skip_wallet_debit: false,
        };
        if byok.is_some() {
            tenant.skip_wallet_debit = true;
        }

        match request.kind {
            crate::agents::AgentKind::Chat => {
                agent_loop::progress::emit_work_fact(
                    sink,
                    agent_loop::progress::WorkFact::understand(&request.query),
                )
                .await;
                let llm = Self::bind_byok_client(
                    self.chat_llm_client
                        .clone()
                        .or_else(|| self.llm_client.clone()),
                    byok.as_ref(),
                );
                self.run_react_mode("chat", llm, |lp| lp, request, sink, &tenant)
                    .await
            }
            crate::agents::AgentKind::Rag => {
                if request.doc_scope.is_empty() {
                    let _ = sink
                        .emit(AgentEvent::Error {
                            code: "missing_doc_scope".to_string(),
                            message: "RAG mode requires a non-empty doc_scope".to_string(),
                        })
                        .await;
                    return Err(AppError::validation(
                        "missing_doc_scope",
                        "RAG mode requires a non-empty doc_scope",
                    ));
                }

                let rag = match self.rag_runtime.clone() {
                    Some(rag) => {
                        // Clone the inner runtime (all fields are Arc-backed, so
                        // cheap) and attach the per-request tenant identity so the
                        // agent-loop retrieval tools (dense/graph) meter their
                        // embedding calls via the configured usage_observer.
                        Arc::new((*rag).clone().with_tenant(tenant.clone()))
                    }
                    None => {
                        let _ = sink
                            .emit(AgentEvent::Error {
                                code: "rag_unavailable".to_string(),
                                message: "RAG runtime is not configured".to_string(),
                            })
                            .await;
                        return Err(AppError::validation(
                            "rag_runtime_not_configured",
                            "RAG runtime is not configured",
                        ));
                    }
                };

                // Dual rag+search: also attach web search when capability present.
                let dual_search = metadata_has_capability(&request, "search");
                let search_executor = if dual_search {
                    match self.search_executor.clone() {
                        Some(executor) => Some(executor),
                        None => {
                            let _ = sink
                                .emit(AgentEvent::Error {
                                    code: "search_unavailable".to_string(),
                                    message: "Search executor is not configured".to_string(),
                                })
                                .await;
                            return Err(AppError::internal("Search executor is not configured"));
                        }
                    }
                } else {
                    None
                };

                agent_loop::progress::emit_work_fact(
                    sink,
                    agent_loop::progress::WorkFact::understand(&request.query),
                )
                .await;
                let llm = Self::bind_byok_client(self.llm_client.clone(), byok.as_ref());
                self.run_react_mode(
                    "rag",
                    llm,
                    |lp| {
                        let mut lp = lp.with_rag_runtime(Some(rag));
                        if let Some(search) = search_executor {
                            lp = lp.with_search_executor(Some(search));
                        }
                        lp
                    },
                    request,
                    sink,
                    &tenant,
                )
                .await
            }
            crate::agents::AgentKind::Search => {
                let search_executor = match self.search_executor.clone() {
                    Some(executor) => executor,
                    None => {
                        let _ = sink
                            .emit(AgentEvent::Error {
                                code: "search_unavailable".to_string(),
                                message: "Search executor is not configured".to_string(),
                            })
                            .await;
                        return Err(AppError::internal("Search executor is not configured"));
                    }
                };

                // SaC search: web via SearchProvider; optional dense via RagRuntime.
                let rag_for_dense = self
                    .rag_runtime
                    .clone()
                    .map(|rag| Arc::new((*rag).clone().with_tenant(tenant.clone())));

                agent_loop::progress::emit_work_fact(
                    sink,
                    agent_loop::progress::WorkFact::understand(&request.query),
                )
                .await;
                let llm = Self::bind_byok_client(
                    self.search_llm_client
                        .clone()
                        .or_else(|| self.llm_client.clone()),
                    byok.as_ref(),
                );
                self.run_react_mode(
                    "search",
                    llm,
                    |lp| {
                        let mut lp = lp.with_search_executor(Some(search_executor));
                        if let Some(rag) = rag_for_dense {
                            lp = lp.with_rag_runtime(Some(rag));
                        }
                        lp
                    },
                    request,
                    sink,
                    &tenant,
                )
                .await
            }
            crate::agents::AgentKind::Write => Err(AppError::validation(
                "write_routed_outside_unified_agent",
                "Write mode is dispatched via chat::pipeline_steps → writer::run_write_mode, not UnifiedAgent",
            )),
        }
    }
}

impl UnifiedAgent {
    /// Common ReAct-mode execution path shared by Chat / Rag / Search.
    ///
    /// Prefers `request.metadata["assembled_mode_config"]` when present (CapabilitySet
    /// assembly path); otherwise loads YAML via `mode_id` (tests / backward compat).
    /// Per-mode differences are confined to the caller: which LLM field is used and
    /// how the loop is configured.
    async fn run_react_mode(
        &self,
        mode_id: &str,
        llm_client: Option<LlmClient>,
        configure_loop: impl FnOnce(agent_loop::r#loop::ReActLoop) -> agent_loop::r#loop::ReActLoop,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
        tenant: &TenantContext,
    ) -> Result<AgentRunResult, AppError> {
        let mode = match resolve_mode_config(mode_id, &request) {
            Ok(m) => m,
            Err(e) => {
                let _ = sink
                    .emit(AgentEvent::Error {
                        code: "mode_config_load_failed".to_string(),
                        message: format!("Failed to load {mode_id} mode config: {e}"),
                    })
                    .await;
                return Err(e);
            }
        };
        let stage_id = if mode.id.is_empty() {
            mode_id.to_string()
        } else {
            mode.id.clone()
        };

        let llm = match llm_client {
            Some(client) => {
                // Tag stage with assembled/legacy mode id; attach exit metering.
                let client = client.with_stage(&stage_id).with_request_context(
                    request
                        .session_id
                        .as_deref()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok()),
                    request.auth.request_id().map(|s| s.to_string()),
                );
                let client = if let Some(ref observer) = self.usage_observer {
                    client.with_observer(observer.clone(), tenant.clone())
                } else {
                    client
                };
                Arc::new(client)
            }
            None => {
                let _ = sink
                    .emit(AgentEvent::Error {
                        code: "llm_unavailable".to_string(),
                        message: "LLM client is not configured".to_string(),
                    })
                    .await;
                return Err(AppError::internal("LLM client is not configured"));
            }
        };

        let skill_registry = Arc::new(agent_tools::capability::CapabilityRegistry::standard());
        let loop_agent = configure_loop(
            agent_loop::r#loop::ReActLoop::new(llm, skill_registry)
                .with_chat_persistence(self.chat_persistence.clone()),
        );
        let mut result = loop_agent.run(&mode, request, sink).await?;
        result.routing_decision = Some(stage_id);
        Ok(result)
    }
}

/// Prefer pipeline-assembled ModeConfig from metadata; fall back to YAML load.
fn resolve_mode_config(
    mode_id: &str,
    request: &AgentRequest,
) -> Result<agent_loop::r#loop::config::ModeConfig, AppError> {
    if let Some(value) = request.metadata.get("assembled_mode_config") {
        if let Ok(cfg) =
            serde_json::from_value::<agent_loop::r#loop::config::ModeConfig>(value.clone())
        {
            // Reject empty shell objects that failed serialization to a usable config.
            if !cfg.id.is_empty() || !cfg.tool_pool.is_empty() || !cfg.system_prompt_base.is_empty()
            {
                return Ok(cfg);
            }
        }
    }
    agent_loop::r#loop::config::load_mode_config(mode_id)
}

fn metadata_has_capability(request: &AgentRequest, cap: &str) -> bool {
    request
        .metadata
        .get("capabilities")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .any(|s| s.eq_ignore_ascii_case(cap))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_llm() -> LlmClient {
        LlmClient::new(avrag_llm::ModelProviderConfig {
            base_url: "http://localhost".to_string(),
            api_key: "dummy".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 1000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
    }

    #[test]
    fn test_unified_agent_builder() {
        let llm = dummy_llm();
        let agent = UnifiedAgent::new(Some(llm.clone()), None, None)
            .with_rag_runtime(None)
            .with_search_executor(None);
        assert!(agent.llm_client.is_some());
        assert!(agent.rag_runtime.is_none());
        assert!(agent.search_executor.is_none());
    }

    fn dummy_secret() -> app_core::ResolvedProviderSecret {
        app_core::ResolvedProviderSecret {
            id: Uuid::nil(),
            owner_user_id: Uuid::nil(),
            workspace_id: None,
            purpose: app_core::ProviderSecretPurpose::Llm,
            provider: "custom".to_string(),
            base_url: Some("http://127.0.0.1:9".to_string()),
            model_hint: Some("e2e-dummy".to_string()),
            api_key: "e2e-not-a-real-key".to_string(),
        }
    }

    #[test]
    fn bind_byok_constructs_client_when_no_platform_client() {
        assert!(UnifiedAgent::bind_byok_client(None, Some(&dummy_secret())).is_some());
    }

    #[test]
    fn bind_byok_overlays_existing_client() {
        assert!(UnifiedAgent::bind_byok_client(Some(dummy_llm()), Some(&dummy_secret())).is_some());
    }

    #[test]
    fn bind_byok_returns_none_without_client_or_secret() {
        assert!(UnifiedAgent::bind_byok_client(None, None).is_none());
    }

    #[test]
    fn llm_client_from_secret_requires_complete_secret() {
        let mut no_url = dummy_secret();
        no_url.base_url = None;
        assert!(llm_client_from_secret(&no_url).is_none());

        let mut no_model = dummy_secret();
        no_model.model_hint = None;
        assert!(llm_client_from_secret(&no_model).is_none());

        let mut no_key = dummy_secret();
        no_key.api_key = String::new();
        assert!(llm_client_from_secret(&no_key).is_none());
    }

    #[test]
    fn assembled_mode_config_roundtrip_keeps_mandatory_codegen() {
        // Probe for the 2026-07-19 eval finding: orchestrator RAG workers ran
        // without the codegen SDK in their retrieve prompt.
        let assembled = crate::assemble_mode(crate::capabilities::CapabilitySet {
            rag: true,
            search: false,
        })
        .expect("assemble rag");
        assert!(
            assembled
                .config
                .skill_catalog
                .mandatory
                .retrieve
                .iter()
                .any(|s| s == "knowledge-base"),
            "assemble_mode lost mandatory codegen: {:?}",
            assembled.config.skill_catalog.mandatory.retrieve
        );
        // Same JSON round-trip the orchestrator worker request goes through
        // (host.rs run_channel → metadata → resolve_mode_config).
        let value = serde_json::to_value(&assembled.config).expect("serialize");
        let cfg: agent_loop::r#loop::config::ModeConfig =
            serde_json::from_value(value).expect("deserialize");
        assert!(
            cfg.skill_catalog
                .mandatory
                .retrieve
                .iter()
                .any(|s| s == "knowledge-base"),
            "metadata round-trip lost mandatory codegen"
        );
    }
}
