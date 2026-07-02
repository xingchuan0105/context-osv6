use std::sync::Arc;

pub mod answer_contract;
pub mod assembler;
pub mod fallback;
pub mod policy;
pub use policy::LoopPolicy;
pub use policy::config;
pub use policy::disclosure_plan;
pub use policy::exit_policy;
pub mod cancellation;
pub use cancellation::DegradeReason;
pub(crate) use cancellation::cancellation_error;
pub mod hooks;
pub mod iteration;
mod iteration_codegen;
mod iteration_tools;
mod message_format;
pub mod message_queue;
pub mod parse;
mod rag_bridge;
pub mod reasoning_emit;
mod run_fallback;
mod run_prepare;
mod run_result;
mod run_retrieval;
mod run_synthesis;
pub mod skill_request;
pub mod skills;
pub mod synthesis;
pub mod telemetry;

pub(crate) use message_format::{
    build_assistant_message_with_tool_calls, build_tool_message, truncate_observation,
    truncate_preview,
};
pub(crate) use rag_bridge::dispatch_rag_tool;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::AgentEventSink;
use crate::agents::runtime::{AgentRequest, AgentRunResult};
use app_core::ChatPersistencePort;
use assembler::DisclosedState;
use avrag_llm::LlmClient;
use common::AppError;
use config::ModeConfig;
use hooks::StandardLoopHooks;
use iteration::IterationState;

pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            rag_runtime: None,
            search_executor: None,
            chat_persistence: None,
            code_interpreter: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    fn effective_chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.chat_persistence.clone().or_else(|| {
            self.rag_runtime
                .as_ref()
                .and_then(|runtime| runtime.chat_persistence())
        })
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.search_executor = executor;
        self
    }

    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let cancel = request.cancellation_token.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(cancellation_error());
        }
        let loop_exit = mode.loop_exit_for_mode();
        let hooks = StandardLoopHooks::default();

        let (request, base_message_count, max_iterations, auth, loop_user_query) =
            self.prepare_run_request(mode, request, sink).await?;

        let mut state = IterationState {
            messages: self.build_initial_messages(mode, &request, &loop_user_query),
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
        };
        let (iteration, direct_answer, telemetry_records, total_usage) = self
            .run_retrieval_loop(
                mode,
                &request,
                &auth,
                &loop_exit,
                &hooks,
                base_message_count,
                max_iterations,
                &cancel,
                &mut state,
                sink,
            )
            .await?;

        let mut messages = state.messages;
        let mut disclosed_state = state.disclosed;
        let mut collected_tool_results = state.tool_results;
        let total_tool_calls = state.total_tool_calls;
        let reasoning_summary_acc = state.reasoning_acc;

        if cancel.is_cancelled() {
            return Err(cancellation_error());
        }

        let retrieval_query = request.query.clone();
        if let Some(result) = self
            .resolve_synthesis_gate(
                mode,
                &loop_exit,
                &request,
                &auth,
                &retrieval_query,
                direct_answer.as_deref(),
                &mut messages,
                &mut collected_tool_results,
                &disclosed_state,
                sink,
                iteration,
                max_iterations,
                total_tool_calls,
                &telemetry_records,
                &total_usage,
                &reasoning_summary_acc,
                start_time,
            )
            .await?
        {
            return Ok(result);
        }

        self.run_synthesis_phase(
            mode,
            &request,
            &mut disclosed_state,
            &messages,
            &collected_tool_results,
            sink,
            &cancel,
            iteration,
            max_iterations,
            total_tool_calls,
            &telemetry_records,
            &total_usage,
            &reasoning_summary_acc,
            start_time,
        )
        .await
    }
}

#[cfg(test)]
mod tests;
