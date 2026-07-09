mod assemble;
mod content_dispatch;
mod state;

pub use state::{IterationControl, IterationOutcome, IterationState};

pub(crate) use content_dispatch::iteration_llm_usage;
pub(crate) use state::disclosed_skill_ids;

use avrag_llm::LlmUsage;
use common::AppError;

use super::ReActLoop;
use super::config::{LoopExitConfig, ModeConfig};
use super::parse::{LlmOutput, parse_llm_output};
use super::skill_request::validate_skill_request;
use crate::agents::events::AgentEventSink;
use crate::agents::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn run_iteration(
        &self,
        iteration: u8,
        max_iterations: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        total_usage: &mut LlmUsage,
        sink: &dyn AgentEventSink,
    ) -> Result<IterationOutcome, AppError> {
        let assembled = self
            .assemble_retrieve_context(iteration, max_iterations, mode, request, state, sink)
            .await;
        let iter_start = std::time::Instant::now();
        let llm_response = self
            .call_retrieve_llm(mode, state, total_usage, &assembled, sink)
            .await?;

        self.apply_llm_output(
            iteration,
            mode,
            request,
            auth,
            loop_exit,
            state,
            sink,
            &llm_response,
            iter_start,
        )
        .await
    }

    pub(crate) async fn apply_llm_output(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        llm_response: &avrag_llm::LlmResponse,
        iter_start: std::time::Instant,
    ) -> Result<IterationOutcome, AppError> {
        let validated = validate_skill_request(mode, &llm_response.content);
        if !validated.is_empty() {
            state.disclosed.last_skill_request = Some(validated);
        }

        let parsed = parse_llm_output(llm_response);

        match parsed {
            LlmOutput::NativeToolCalls(calls) => {
                self.dispatch_native_tool_calls(
                    iteration,
                    mode,
                    request,
                    auth,
                    loop_exit,
                    state,
                    sink,
                    llm_response,
                    iter_start,
                    calls,
                )
                .await
            }
            LlmOutput::CodeBlocks(codes) => {
                self.dispatch_codegen(
                    iteration,
                    request,
                    auth,
                    state,
                    sink,
                    llm_response,
                    iter_start,
                    codes,
                )
                .await
            }
            LlmOutput::Content(content) => {
                self.dispatch_content(
                    iteration,
                    mode,
                    loop_exit,
                    state,
                    sink,
                    llm_response,
                    iter_start,
                    content,
                )
                .await
            }
        }
    }
}

#[cfg(test)]
mod tests;
