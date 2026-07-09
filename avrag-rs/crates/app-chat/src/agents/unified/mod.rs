//! UnifiedAgent — single agent implementation that routes between
//! Chat / RAG / Search modes via `AgentRequest.kind`.
//!
//! v6 (ADR-0006): Chat / RAG / Search route through the unified `ReActLoop`
//! (`crate::agents::loop`). Differences between modes are expressed through
//! YAML `ModeConfig` files (`modes/chat.yaml`, `modes/rag.yaml`, `modes/search.yaml`)
//! rather than independent Strategy state machines.
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

use app_core::ChatPersistencePort;
use avrag_llm::{LlmClient, TenantContext, UsageObserver};
use avrag_search::SearchProvider;
use common::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub use agent_loop::helpers;
pub use agent_tools::weather;

/// Unified agent that dispatches to Chat / RAG / Search based on `request.kind`.
pub struct UnifiedAgent {
    llm_client: Option<LlmClient>,
    chat_llm_client: Option<LlmClient>,
    search_llm_client: Option<LlmClient>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    usage_observer: Option<Arc<dyn UsageObserver>>,
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
        let org_id = request.auth.org_id().to_string();
        let actor_id_owned = request
            .auth
            .actor_id()
            .map(|id| id.into_uuid().to_string());
        let audit_record = audit::routing_decision_record(
            &org_id,
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

        let tenant = TenantContext {
            org_id: request.auth.org_id().into_uuid(),
            user_id: request
                .auth
                .actor_id()
                .map(|id| id.into_uuid())
                .unwrap_or_else(Uuid::nil),
        };

        match request.kind {
            crate::agents::AgentKind::Chat => {
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "chat".to_string(),
                        message: "ReAct chat".to_string(),
                    })
                    .await;
                self.run_react_mode(
                    "chat",
                    self.chat_llm_client.clone().or_else(|| self.llm_client.clone()),
                    |lp| lp,
                    request,
                    sink,
                    &tenant,
                )
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

                self.run_react_mode(
                    "rag",
                    self.llm_client.clone(),
                    |lp| lp.with_rag_runtime(Some(rag)),
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

                self.run_react_mode(
                    "search",
                    self.search_llm_client
                        .clone()
                        .or_else(|| self.llm_client.clone()),
                    |lp| lp.with_search_executor(Some(search_executor)),
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
    /// Loads the mode config, resolves the supplied `llm_client` (attaching the
    /// usage observer when present), builds the loop via `configure_loop`, runs
    /// it, and stamps the routing decision. Per-mode differences are confined to
    /// the caller: which LLM field is used and how the loop is configured.
    async fn run_react_mode(
        &self,
        mode_id: &str,
        llm_client: Option<LlmClient>,
        configure_loop: impl FnOnce(agent_loop::r#loop::ReActLoop) -> agent_loop::r#loop::ReActLoop,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
        tenant: &TenantContext,
    ) -> Result<AgentRunResult, AppError> {
        let mode = match agent_loop::r#loop::config::load_mode_config(mode_id) {
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

        let llm = match llm_client {
            Some(client) => {
                // Tag stage with mode id; attach exit metering when configured.
                let client = client.with_stage(mode_id);
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
        result.routing_decision = Some(mode_id.to_string());
        Ok(result)
    }
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
}
