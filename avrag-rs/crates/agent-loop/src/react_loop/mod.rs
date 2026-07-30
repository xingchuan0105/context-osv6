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
pub mod deps;
pub mod hooks;
pub mod iteration;
mod iteration_codegen;
pub mod session_fs;
pub mod sdk_gate;
mod iteration_tools;
pub mod json_fence;
mod message_format;
pub mod message_queue;
pub mod parse;
pub mod prompt_assets;
// rag_bridge moved to agent-tools (TN Wave 6)
pub use agent_tools::rag_bridge;
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
use agent_tools::capability::CapabilityRegistry;
use crate::events::AgentEventSink;
use crate::runtime::{AgentRequest, AgentRunResult};
use app_core::ChatPersistencePort;
use assembler::DisclosedState;
use avrag_llm::LlmClient;
use common::AppError;
use config::ModeConfig;
use iteration::IterationState;

pub use deps::{BridgeCallObs, LoopRuntimeDeps};
pub use sdk_gate::{method_allowed, sdk_primitives_for_caps};
pub use hooks::{BeforeToolCallOutcome, LoopContext, LoopHooks, StandardLoopHooks};

/// ReAct retrieve → gate → synthesis engine.
///
/// Runtime side-effects (rag/search/codegen) live in [`LoopRuntimeDeps`]; prefer
/// `with_*` builders over reaching into `deps` from product code.
pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    deps: LoopRuntimeDeps,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            deps: LoopRuntimeDeps::default(),
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.deps.chat_persistence = chat_persistence;
        self
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.deps.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.deps.search_executor = executor;
        self
    }

    /// Replace the whole deps bag (tests / advanced composition).
    pub fn with_runtime_deps(mut self, deps: LoopRuntimeDeps) -> Self {
        self.deps = deps;
        self
    }

    pub fn runtime_deps(&self) -> &LoopRuntimeDeps {
        &self.deps
    }

    /// Run with default [`StandardLoopHooks`] (two-tier compact: high=32, low=20).
    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        self.run_with_hooks(mode, request, sink, &StandardLoopHooks::default())
            .await
    }

    /// Run with an injected [`LoopHooks`] implementation.
    ///
    /// Prefer this over forking `run` for context transforms. Hooks are **not**
    /// the tool-policy truth source — see `EXTENDING.md` / `hooks` module docs.
    pub async fn run_with_hooks(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
        hooks: &dyn LoopHooks,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let cancel = request.cancellation_token.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(cancellation_error());
        }
        let loop_exit = mode.loop_exit_for_mode();

        let (request, base_message_count, max_iterations, auth, loop_user_query) =
            self.prepare_run_request(mode, request, sink).await?;

        // W1 (2026-07-28, channel-persistent worker): a resumed worker session
        // passes its alias cursor so retrieval-log aliases stay unique across
        // briefs of the same turn (see worker_contract::RETRIEVAL_ALIAS_START_METADATA).
        let alias_start = request
            .metadata
            .get(crate::worker_contract::RETRIEVAL_ALIAS_START_METADATA)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut state = IterationState {
            messages: self.build_initial_messages(mode, &request, &loop_user_query),
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
            answer_deltas_streamed: false,
            compile_continuations: 0,
            retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(alias_start)),
            session_fs: std::sync::Arc::new(session_fs::SessionFs::new()),
            sdk_allowed: std::sync::Arc::new(
                mode.sdk_primitives.iter().cloned().collect(),
            ),
        };
        let (iteration, direct_answer, telemetry_records, total_usage) = self
            .run_retrieval_loop(
                mode,
                &request,
                &auth,
                &loop_exit,
                hooks,
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
        let answer_deltas_streamed = state.answer_deltas_streamed;

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
                answer_deltas_streamed,
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
