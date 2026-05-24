//! UnifiedAgent — single agent implementation that routes between
//! Chat / RAG / Search modes via `AgentRequest.kind`.
//!
//! v5: Each mode is driven by its own Strategy state machine through the
//! generic StrategyExecutor. Old ProgressiveLoop + LoopAdapter code removed.

use crate::agents::audit;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{LoopBudget, UserTier};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use crate::agents::strategy::{
    executor::StrategyExecutor,
    chat::{ChatContext, ChatStrategy},
    rag::{RagContext, RagStrategy},
    search::{SearchContext, SearchStrategy},
};
use avrag_llm::LlmClient;
use avrag_search::SearchProvider;
use common::AppError;
use std::sync::Arc;

pub mod atomic_tools;
pub mod helpers;
pub mod weather;

/// Unified agent that dispatches to Chat / RAG / Search based on `request.kind`.
pub struct UnifiedAgent {
    llm_client: Option<LlmClient>,
    llm_provider: Option<Arc<dyn avrag_llm::LlmProvider>>,
    temperature: Option<f32>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn SearchProvider>>,
}

impl UnifiedAgent {
    pub fn new(
        llm_client: Option<LlmClient>,
        temperature: Option<f32>,
    ) -> Self {
        let llm_provider = llm_client.clone().map(|c| {
            Arc::new(c) as Arc<dyn avrag_llm::LlmProvider>
        });
        Self {
            llm_client,
            llm_provider,
            temperature,
            rag_runtime: None,
            search_executor: None,
        }
    }

    /// Override the LLM provider (used by E2E tests to inject RecordingLlmProvider).
    pub fn with_llm_provider(mut self, provider: Option<Arc<dyn avrag_llm::LlmProvider>>) -> Self {
        self.llm_provider = provider;
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
}

#[async_trait::async_trait]
impl Agent for UnifiedAgent {
    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let cancellation = request.cancellation_token.clone().unwrap_or_default();
        let trace_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| "unified-agent".to_string());

        let llm_provider = match self.llm_provider.clone() {
            Some(provider) => provider,
            None => {
                let _ = sink.emit(AgentEvent::Error {
                    code: "llm_unavailable".to_string(),
                    message: "LLM client is not configured".to_string(),
                }).await;
                return Err(AppError::internal("LLM client is not configured"));
            }
        };
        let llm_client = self.llm_client.clone();

        // v5: RouterPolicy produces an observable routing decision.
        let router_policy = crate::agents::capability::standard_policy();
        let routing_decision = router_policy.resolve(&request);
        let _ = sink.emit(AgentEvent::RoutingDecision {
            strategy_id: routing_decision.strategy_id.clone(),
            matched_rule: routing_decision.matched_rule.clone(),
            confidence: routing_decision.confidence,
            explanation: routing_decision.explanation.clone(),
        }).await;

        // Emit audit record for routing decision.
        let org_id = request
            .auth_context
            .get("org_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let actor_id = request.auth_context.get("actor_id").and_then(|v| v.as_str());
        let audit_record = audit::routing_decision_record(
            org_id,
            actor_id,
            &trace_id,
            &routing_decision.strategy_id,
            &routing_decision.matched_rule,
            routing_decision.confidence,
            &routing_decision.explanation,
        );
        let _ = sink
            .emit(AgentEvent::Audit {
                record: audit_record,
            })
            .await;

        let executor = StrategyExecutor;

        match request.kind {
            crate::agents::AgentKind::Chat => {
                let _ = sink.emit(AgentEvent::Activity {
                    stage: "chat".to_string(),
                    message: "Direct chat".to_string(),
                }).await;

                let ctx = ChatContext::from_request(
                    request,
                    trace_id,
                    LoopBudget::chat(UserTier::Pro),
                    sink.clone_boxed(),
                    cancellation,
                )?;
                let strategy = ChatStrategy {
                    llm: llm_provider.clone(),
                    llm_client: llm_client.clone(),
                    temperature: self.temperature,
                };
                let mut result = executor.run(&strategy, ctx).await?;
                result.routing_decision = Some(routing_decision.clone());
                Ok(result)
            }
            crate::agents::AgentKind::Rag => {
                if request.doc_scope.is_empty() {
                    let _ = sink.emit(AgentEvent::Error {
                        code: "missing_doc_scope".to_string(),
                        message: "RAG mode requires a non-empty doc_scope".to_string(),
                    }).await;
                    return Err(AppError::validation(
                        "missing_doc_scope",
                        "RAG mode requires a non-empty doc_scope",
                    ));
                }

                let rag = match self.rag_runtime.clone() {
                    Some(rag) => rag,
                    None => {
                        let _ = sink.emit(AgentEvent::Error {
                            code: "rag_unavailable".to_string(),
                            message: "RAG runtime is not configured".to_string(),
                        }).await;
                        return Err(AppError::internal("RAG runtime is not configured"));
                    }
                };

                let ctx = RagContext::from_request(
                    request,
                    trace_id,
                    LoopBudget::rag(UserTier::Pro),
                    sink.clone_boxed(),
                    cancellation,
                    rag,
                )?;
                let strategy = RagStrategy {
                    llm: llm_provider.clone(),
                    llm_client: llm_client.clone(),
                    temperature: self.temperature,
                };
                let mut result = executor.run(&strategy, ctx).await?;
                result.routing_decision = Some(routing_decision.clone());
                Ok(result)
            }
            crate::agents::AgentKind::Search => {
                let search_executor = match self.search_executor.clone() {
                    Some(executor) => executor,
                    None => {
                        let _ = sink.emit(AgentEvent::Error {
                            code: "search_unavailable".to_string(),
                            message: "Search executor is not configured".to_string(),
                        }).await;
                        return Err(AppError::internal("Search executor is not configured"));
                    }
                };

                let ctx = SearchContext::from_request(
                    request,
                    trace_id,
                    LoopBudget::search(UserTier::Pro),
                    sink.clone_boxed(),
                    cancellation,
                )?;
                let strategy = SearchStrategy {
                    llm: llm_provider.clone(),
                    llm_client: llm_client.clone(),
                    temperature: self.temperature,
                    search_executor,
                    search_synthesizer: llm_client.clone().map(|llm_client| {
                        let llm: Arc<dyn avrag_llm::LlmProvider> = Arc::new(llm_client.clone());
                        Arc::new(crate::agents::strategy::search::LlmSearchAnswerSynthesizer {
                            llm,
                            llm_client: Some(llm_client),
                        }) as Arc<dyn crate::agents::strategy::search::SearchAnswerSynthesizer>
                    }),
                };
                let mut result = executor.run(&strategy, ctx).await?;
                result.routing_decision = Some(routing_decision.clone());
                Ok(result)
            }
        }
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
        let agent = UnifiedAgent::new(Some(llm.clone()), Some(0.7))
            .with_rag_runtime(None)
            .with_search_executor(None);
        assert!(agent.llm_client.is_some());
        assert_eq!(agent.temperature, Some(0.7));
        assert!(agent.rag_runtime.is_none());
        assert!(agent.search_executor.is_none());
    }
}
