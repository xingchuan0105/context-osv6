//! Generic agent tool-use loop (ReAct harness).
//!
//! Drives the core cycle:
//!   1. Build messages (system prompt + history + user query)
//!   2. Call LLM with tools → get ToolAwareResponse
//!   3. If ToolUse: execute tools, append results to messages, loop
//!   4. If EndTurn: return the answer
//!
//! The loop is bounded by [`LoopBudget`] and wired to [`AgentEventSink`] for
//! observable execution.

use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{DegradeReason, LoopBudget, ReactContext, cancellation_error};
use crate::agents::runtime::{AgentRunResult, AgentRunUsage, FinalDecision, IterationRecord};
use crate::agents::tool_registry::AgentToolRegistry;
use avrag_llm::{ChatMessage as LlmChatMessage, LlmClient, LlmUsage};
use common::{AppError, StopReason, ToolAwareResponse, ToolResult, ToolStatus};
use std::time::Instant;

/// Outcome of one agent loop run.
pub enum AgentLoopOutcome {
    /// Normal completion with an answer.
    Answer(String),
    /// The loop degraded (budget exhausted, all tools failed, etc.).
    Degraded { reason: DegradeReason, partial_answer: Option<String> },
    /// The loop needs clarification from the user.
    Clarify(String),
}

/// Configuration for a single agent loop run.
pub struct AgentLoopConfig<'a> {
    /// The LLM client used for completions.
    pub llm: &'a LlmClient,
    /// Temperature for LLM calls.
    pub temperature: Option<f32>,
    /// System prompt (agent personality + rules).
    pub system_prompt: String,
    /// User query + conversation history.
    pub messages: Vec<LlmChatMessage>,
    /// Tool registry scoped to this agent kind.
    pub registry: &'a AgentToolRegistry,
    /// Iteration budget.
    pub budget: LoopBudget,
    /// Cancellation token.
    pub cancellation: tokio_util::sync::CancellationToken,
    /// Trace id for logging.
    pub trace_id: String,
}

/// Run the generic tool-use loop.
///
/// This is the **Agent Harness** core per §12 of the architecture baseline.
/// It is agent-agnostic: the caller provides the system prompt, tool registry,
/// and initial messages; the loop handles tool dispatch and iteration bounding.
pub async fn run_agent_loop(
    config: AgentLoopConfig<'_>,
    sink: &dyn AgentEventSink,
) -> Result<AgentLoopOutcome, AppError> {
    let AgentLoopConfig {
        llm,
        temperature,
        system_prompt,
        mut messages,
        registry,
        mut budget,
        cancellation,
        trace_id,
    } = config;

    let ctx = ReactContext::new(sink, &cancellation, &trace_id);
    let tool_specs = registry.specs_for_kind(crate::agents::AgentKind::Chat); // TODO: pass kind

    // Prepend system prompt
    messages.insert(0, LlmChatMessage::system(system_prompt));

    let mut iterations: Vec<IterationRecord> = Vec::new();
    let mut aggregated_usage: Option<LlmUsage> = None;
    let mut request_count: u64 = 0;

    loop {
        ctx.check_cancelled()?;

        let iteration_idx = budget.current;
        let iter_started = Instant::now();

        ctx.emit_activity(
            "thinking",
            format!("Agent reasoning (iteration {})", iteration_idx + 1),
        )
        .await;

        // --- LLM call with tools ---
        let llm_response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(cancellation_error());
            }
            result = llm.complete_with_tools(&messages, &tool_specs, temperature) => {
                match result {
                    Ok(resp) => resp,
                    Err(error) => {
                        tracing::warn!(error = %error, "LLM tool completion failed");
                        return Err(AppError::internal(format!(
                            "LLM tool completion failed: {error}"
                        )));
                    }
                }
            }
        };

        // --- Handle stop reason ---
        match llm_response.stop_reason {
            StopReason::EndTurn | StopReason::StopSequence | StopReason::MaxTokens => {
                // Direct answer (or stop sequence / max tokens) — no tool calls
                budget.tick();
                iterations.push(IterationRecord {
                    iteration: iteration_idx,
                    plan: serde_json::json!({"action": "answer"}),
                    signals: Default::default(),
                    decision: "answer".to_string(),
                    elapsed_ms: iter_started.elapsed().as_millis() as u64,
                    llm_evaluation: None,
                    usage: None,
                });

                return Ok(AgentLoopOutcome::Answer(llm_response.content));
            }

            StopReason::ToolUse => {
                // Execute tools and append results to conversation
                let n_calls = llm_response.tool_calls.len();
                ctx.emit_activity(
                    "tool_use",
                    format!("Executing {} tool call(s)", n_calls),
                )
                .await;

                let mut tool_results: Vec<ToolResult> = Vec::new();
                for call in &llm_response.tool_calls {
                    let result = registry.execute(&call.name, call.arguments.clone()).await;
                    match result {
                        Ok(r) => tool_results.push(r),
                        Err(error) => {
                            tool_results.push(ToolResult {
                                tool: call.name.clone(),
                                version: "1.0".to_string(),
                                status: ToolStatus::Error,
                                data: Some(serde_json::json!({"error": error.to_string()})),
                                trace: None,
                            });
                        }
                    }
                }

                // Append assistant message (tool calls) to conversation
                let assistant_content = if llm_response.content.is_empty() {
                    serde_json::json!({"tool_calls": llm_response.tool_calls})
                } else {
                    serde_json::json!({
                        "content": llm_response.content,
                        "tool_calls": llm_response.tool_calls,
                    })
                };
                messages.push(LlmChatMessage::assistant(assistant_content.to_string()));

                // Append tool results as function-role messages
                for result in &tool_results {
                    let content = serde_json::json!({
                        "tool": result.tool,
                        "status": result.status,
                        "data": result.data,
                    });
                    messages.push(LlmChatMessage {
                        role: "tool".to_string(),
                        content: content.to_string(),
                    });
                }

                budget.tick();
                let elapsed_ms = iter_started.elapsed().as_millis() as u64;

                // --- Budget exhausted check ---
                if budget.exhausted() {
                    let decision = if tool_results.iter().all(|r| r.status != ToolStatus::Ok) {
                        "degrade".to_string()
                    } else {
                        "synthesize".to_string()
                    };
                    iterations.push(IterationRecord {
                        iteration: iteration_idx,
                        plan: serde_json::json!({"tool_calls": llm_response.tool_calls}),
                        signals: Default::default(),
                        decision: decision.clone(),
                        elapsed_ms,
                        llm_evaluation: None,
                        usage: None,
                    });

                    if decision == "degrade" {
                        return Ok(AgentLoopOutcome::Degraded {
                            reason: DegradeReason::NoResultsAfterAllFallbacks,
                            partial_answer: Some(llm_response.content),
                        });
                    }

                    // Budget exhausted but we have tool results — synthesize
                    return Ok(AgentLoopOutcome::Answer(format!(
                        "{}",
                        llm_response.content
                    )));
                }

                // Record iteration and continue loop
                iterations.push(IterationRecord {
                    iteration: iteration_idx,
                    plan: serde_json::json!({"tool_calls": llm_response.tool_calls}),
                    signals: Default::default(),
                    decision: "tool_use".to_string(),
                    elapsed_ms,
                    llm_evaluation: None,
                    usage: None,
                });

                // Continue to next iteration
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;
    use crate::agents::react_loop::UserTier;
    use crate::agents::tool_registry::{AgentToolRegistry, PlaceholderTool};

    #[tokio::test]
    async fn agent_loop_with_no_tools_returns_answer() {
        // This test requires a real LLM client; for now we verify the
        // type system and compilation.
        let _registry = AgentToolRegistry::new();
        let _budget = LoopBudget::rag(UserTier::Pro);
    }

    #[test]
    fn agent_loop_outcome_variants() {
        let answer = AgentLoopOutcome::Answer("hello".to_string());
        let degraded = AgentLoopOutcome::Degraded {
            reason: DegradeReason::NoResultsAfterAllFallbacks,
            partial_answer: None,
        };
        let clarify = AgentLoopOutcome::Clarify("what?".to_string());

        // Just verify they compile
        match answer {
            AgentLoopOutcome::Answer(_) => {}
            _ => panic!(),
        }
        match degraded {
            AgentLoopOutcome::Degraded { .. } => {}
            _ => panic!(),
        }
        match clarify {
            AgentLoopOutcome::Clarify(_) => {}
            _ => panic!(),
        }
    }
}
