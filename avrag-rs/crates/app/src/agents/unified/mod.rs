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
    temperature: Option<f32>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn SearchProvider>>,
}

impl UnifiedAgent {
    pub fn new(
        llm_client: Option<LlmClient>,
        temperature: Option<f32>,
    ) -> Self {
        Self {
            llm_client,
            temperature,
            rag_runtime: None,
            search_executor: None,
        }
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

        let llm = match self.llm_client.clone() {
            Some(llm) => llm,
            None => {
                let _ = sink.emit(AgentEvent::Error {
                    code: "llm_unavailable".to_string(),
                    message: "LLM client is not configured".to_string(),
                }).await;
                return Err(AppError::internal("LLM client is not configured"));
            }
        };

        // v5: RouterPolicy produces an observable routing decision.
        let router_policy = crate::agents::capability::standard_policy();
        let routing_decision = router_policy.resolve(&request);
        let _ = sink.emit(AgentEvent::DebugTrace {
            kind: "routing.decision".to_string(),
            payload: serde_json::json!({
                "strategy_id": routing_decision.strategy_id,
                "matched_rule": routing_decision.matched_rule,
                "confidence": routing_decision.confidence,
                "explanation": routing_decision.explanation,
            }),
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
                    llm,
                    temperature: self.temperature,
                };
                executor.run(&strategy, ctx).await
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
                    llm,
                    temperature: self.temperature,
                };
                executor.run(&strategy, ctx).await
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
                    llm,
                    temperature: self.temperature,
                    search_executor,
                    search_synthesizer: self.llm_client.clone().map(|llm| {
                        Arc::new(crate::agents::strategy::search::LlmSearchAnswerSynthesizer { llm })
                            as Arc<dyn crate::agents::strategy::search::SearchAnswerSynthesizer>
                    }),
                };
                executor.run(&strategy, ctx).await
            }
            crate::agents::AgentKind::Composite => {
                let rag_runtime = self.rag_runtime.clone();
                let search_executor = self.search_executor.clone();

                let ctx = crate::agents::strategy::composite::CompositeContext::from_request(
                    request,
                    trace_id,
                    LoopBudget::rag(UserTier::Pro), // composite uses rag-level budget
                    sink.clone_boxed(),
                    cancellation,
                )?;
                let strategy = crate::agents::strategy::composite::CompositeStrategy {
                    llm,
                    temperature: self.temperature,
                    rag_runtime,
                    search_executor,
                };
                executor.run(&strategy, ctx).await
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
