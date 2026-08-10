mod assemble;
mod content_dispatch;
mod state;

pub use state::{IterationControl, IterationOutcome, IterationState};

pub(crate) use content_dispatch::iteration_llm_usage;
pub(crate) use state::{consumes_iteration_budget, disclosed_skill_ids};

use avrag_llm::LlmUsage;
use common::AppError;

use super::ReActLoop;
use super::config::{LoopExitConfig, ModeConfig};
use super::hooks::LoopHooks;
use super::parse::{LlmOutput, parse_llm_output};
use super::skill_request::validate_skill_request;
use crate::events::AgentEventSink;
use crate::runtime::AgentRequest;

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
        hooks: &dyn LoopHooks,
        tokens_max: u32,
    ) -> Result<IterationOutcome, AppError> {
        let assembled = self
            .assemble_retrieve_context(
                iteration,
                max_iterations,
                mode,
                request,
                state,
                sink,
                // Budget hint shows billable (uncached) tokens — same
                // accounting as the loop budget gate.
                total_usage.billable_tokens(),
                tokens_max,
            )
            .await;
        let iter_start = std::time::Instant::now();
        let llm_response = self
            .call_retrieve_llm(mode, request, state, total_usage, &assembled, sink, hooks)
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
            hooks,
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
        hooks: &dyn LoopHooks,
    ) -> Result<IterationOutcome, AppError> {
        let validated = validate_skill_request(mode, &llm_response.content);
        if !validated.is_empty() {
            state.disclosed.last_skill_request = Some(validated);
        }

        // SaC knockout: note seen chunks + register KNOCKOUT lines on every retrieve turn.
        if let Ok(mut ko) = state.knockout.lock() {
            ko.note_seen_from_tool_results(&state.tool_results);
            ko.register_from_model_text(&llm_response.content);
        }
        // EWS: KEEP / KEEP_DROP from model text (sticky when no KEEP line).
        // Applied again after tools so same-turn KEEP can resolve new aliases.
        Self::apply_ews_from_model_text(state, &llm_response.content);

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
                    hooks,
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
                    hooks,
                )
                .await
            }
            LlmOutput::Content(content) => {
                self.dispatch_content(
                    iteration,
                    mode,
                    request,
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

    /// Parse KEEP/KEEP_DROP against current `tool_results` + evidence bodies.
    pub(crate) fn apply_ews_from_model_text(state: &mut IterationState, text: &str) {
        let bodies = state.evidence.seen_chunk_bodies.clone();
        state.ews.apply_from_model_text(text, &state.tool_results, |cid| {
            bodies
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(cid)
                .cloned()
        });
    }
}

#[cfg(test)]
mod tests;
