//! ChatStrategy — v5 state machine for Chat mode.
//!
//! Chat is single-shot with optional atomic-tool execution:
//!   Plan → [ExecuteAtomic] → Answer
//! No evaluation loop.

use super::{AgentErrorKind, State, StateKind, StepOutcome, Strategy, StrategyContext};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::LoopBudget;
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};
use common::{AppError, ToolCall, ToolResult, ToolStatus};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// ChatState
// ---------------------------------------------------------------------------

/// States in the Chat state machine.
#[derive(Debug)]
pub enum ChatState {
    /// Plan: run planner LLM to decide strategy and optional tool calls.
    Plan,
    /// ExecuteAtomic: run atomic tools selected by the planner.
    ExecuteAtomic { calls: Vec<ToolCall> },
    /// Answer: generate the final response (stream or non-stream).
    Answer,
}

impl State for ChatState {
    fn state_id(&self) -> &'static str {
        match self {
            ChatState::Plan => "plan",
            ChatState::ExecuteAtomic { .. } => "execute_atomic",
            ChatState::Answer => "answer",
        }
    }

    fn state_kind(&self) -> StateKind {
        match self {
            ChatState::Plan => StateKind::Plan,
            ChatState::ExecuteAtomic { .. } => StateKind::Execute,
            ChatState::Answer => StateKind::Answer,
        }
    }

    fn to_observable(&self) -> serde_json::Value {
        match self {
            ChatState::Plan => serde_json::json!({"state": "plan"}),
            ChatState::ExecuteAtomic { calls } => {
                serde_json::json!({"state": "execute_atomic", "call_count": calls.len()})
            }
            ChatState::Answer => serde_json::json!({"state": "answer"}),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// ChatContext
// ---------------------------------------------------------------------------

/// Runtime context for ChatStrategy.
pub struct ChatContext {
    pub request: AgentRequest,
    pub trace_id: String,
    pub budget: LoopBudget,
    pub sink: Box<dyn AgentEventSink>,
    pub cancel: CancellationToken,
    pub auth: avrag_auth::AuthContext,
    /// Accumulated tool results (filled during ExecuteAtomic).
    pub tool_results: Vec<ToolResult>,
    /// Plan decision action recorded during Plan (for white-box reporting).
    pub plan_decision_action: Option<String>,
    /// Tool call records for white-box reporting.
    pub tool_call_records: Vec<crate::agents::runtime::ToolCallRecord>,
    /// Accumulated LLM usage across all calls.
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    /// Number of LLM requests made.
    pub request_count: u64,
    /// Degrade trace from content guard sanitization (R3/R6).
    pub content_guard_trace: Vec<common::DegradeTraceItem>,
}

impl StrategyContext for ChatContext {
    fn trace_id(&self) -> &str {
        &self.trace_id
    }

    fn budget(&self) -> &LoopBudget {
        &self.budget
    }

    fn budget_mut(&mut self) -> &mut LoopBudget {
        &mut self.budget
    }

    fn sink(&self) -> &dyn AgentEventSink {
        self.sink.as_ref()
    }

    fn cancel(&self) -> &CancellationToken {
        &self.cancel
    }

    fn org_id(&self) -> Option<String> {
        Some(self.auth.org_id().to_string())
    }

    fn actor_id(&self) -> Option<String> {
        self.auth.actor_id().map(|id| id.uuid().to_string())
    }

    fn request(&self) -> Option<&crate::agents::runtime::AgentRequest> {
        Some(&self.request)
    }
}

impl ChatContext {
    /// Build a ChatContext from an AgentRequest and runtime dependencies.
    pub fn from_request(
        request: AgentRequest,
        trace_id: String,
        budget: LoopBudget,
        sink: Box<dyn AgentEventSink>,
        cancel: CancellationToken,
    ) -> Result<Self, common::AppError> {
        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone()).map_err(|error| {
                common::AppError::internal(format!("Failed to deserialize auth context: {error}"))
            })?;
        Ok(Self {
            request,
            trace_id,
            budget,
            sink,
            cancel,
            auth,
            tool_results: Vec::new(),
            plan_decision_action: None,
            tool_call_records: Vec::new(),
            aggregated_usage: None,
            request_count: 0,
            content_guard_trace: Vec::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// ChatStrategy
// ---------------------------------------------------------------------------

/// Strategy implementation for Chat mode.
pub struct ChatStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub llm_client: Option<avrag_llm::LlmClient>,
    pub temperature: Option<f32>,
}

#[async_trait::async_trait]
impl Strategy for ChatStrategy {
    type Context = ChatContext;

    async fn init(
        &self,
        _ctx: &mut ChatContext,
    ) -> Result<Box<dyn State>, AppError> {
        Ok(Box::new(ChatState::Plan))
    }

    fn schema() -> crate::agents::capability::StrategySchema {
        crate::agents::capability::StrategySchema {
            id: "chat".to_string(),
            states: vec!["Plan".to_string(), "ExecuteAtomic".to_string(), "Answer".to_string()],
            transitions: vec![
                crate::agents::capability::TransitionSchema { from: "Plan".to_string(), to: "ExecuteAtomic".to_string() },
                crate::agents::capability::TransitionSchema { from: "Plan".to_string(), to: "Answer".to_string() },
                crate::agents::capability::TransitionSchema { from: "ExecuteAtomic".to_string(), to: "Answer".to_string() },
            ],
            external_tools_used: vec![],
            requires_internet: false,
            max_budget: 1,
        }
    }

    async fn step(
        &self,
        state: Box<dyn State>,
        ctx: &mut ChatContext,
    ) -> Result<StepOutcome, AgentErrorKind> {
        // Downcast to concrete ChatState.
        let chat_state = state
            .as_any()
            .downcast_ref::<ChatState>()
            .ok_or_else(|| AgentErrorKind::ModelOutputInvalid {
                expected_schema: "ChatState".to_string(),
                got: "unknown state type".to_string(),
            })?;

        match chat_state {
            ChatState::Plan => self.step_plan(ctx).await,
            ChatState::ExecuteAtomic { calls } => self.step_execute(ctx, calls.clone()).await,
            ChatState::Answer => self.step_answer(ctx).await,
        }
    }
}

impl ChatStrategy {
    // --- Plan step ---

    async fn step_plan(&self, ctx: &mut ChatContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;
        ctx.budget.tick();
        let _ = ctx
            .sink
            .emit(AgentEvent::BudgetTick {
                current: ctx.budget.current,
                max: ctx.budget.max_iterations,
            })
            .await;

        // Build system prompt from chat-plan skill + tool catalog.
        let system_prompt = crate::agents::strategy::prompts::build_plan_system_prompt(
            crate::agents::strategy::prompts::chat::PLANNER_SKILL_ID,
            "chat",
        );
        let messages = build_chat_messages_with_system(&ctx.request, &system_prompt);

        // Call planner LLM.
        let plan_response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(AgentErrorKind::Unknown("cancelled".to_string()));
            }
            result = self.llm.complete(&messages, self.temperature) => {
                result.map_err(|_e| AgentErrorKind::ModelUnavailable {
                    provider: "unknown".to_string(),
                    model: "unknown".to_string(),
                })?
            }
        };

        ctx.request_count += 1;
        ctx.aggregated_usage = Some(crate::agents::unified::helpers::merge_usage(
            ctx.aggregated_usage.as_ref(),
            &plan_response.usage,
        ));

        // Parse plan decision.
        let decision = parse_chat_plan_decision(&plan_response.content);
        ctx.plan_decision_action = Some(decision.action.clone());

        let _ = ctx
            .sink
            .emit(AgentEvent::PlanDecision {
                selected_tools: decision.calls.clone(),
                selected_skills: vec![],
                reasoning: format!("plan action: {}", decision.action),
            })
            .await;

        match decision.action.as_str() {
            "clarify" => {
                let question = if decision.clarification_message.is_empty() {
                    "Could you clarify your request?".to_string()
                } else {
                    decision.clarification_message.clone()
                };
                Ok(StepOutcome::Terminate(AgentRunResult {
                    answer: question.clone(),
                    final_decision: Some(FinalDecision::Clarified { question }),
                    decisions: vec![crate::agents::runtime::DecisionRecord {
                        phase: "plan".to_string(),
                        iteration: 0,
                        decision: "clarify".to_string(),
                        reasoning: decision.clarification_message,
                        selected_tools: vec![],
                    }],
                    ..Default::default()
                }))
            }
            _ => {
                if decision.calls.is_empty() {
                    Ok(StepOutcome::Next(Box::new(ChatState::Answer)))
                } else {
                    Ok(StepOutcome::Next(Box::new(ChatState::ExecuteAtomic {
                        calls: decision.calls,
                    })))
                }
            }
        }
    }

    // --- Execute step ---

    async fn step_execute(
        &self,
        ctx: &mut ChatContext,
        calls: Vec<ToolCall>,
    ) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        let results = crate::agents::unified::atomic_tools::dispatch_atomic_tools_with_enforcement(
            calls.clone(),
            None,
            Some(&ctx.auth),
        )
        .await;

        // ① content_guard: R3/R6 — scan tool results for prompt injection.
        let (mut results, guard_trace) = if let Some(ref guard) = ctx.request.guard_pipeline {
            crate::agents::content_guard::sanitize_tool_results(
                &results,
                guard.as_ref(),
                Some(ctx.trace_id.clone()),
            )
        } else {
            (results, Vec::new())
        };
        ctx.content_guard_trace.extend(guard_trace);

        // ② UntrustedInputProcessor: R17 — heuristic injection detection + structured wrapping.
        let mut rejected = Vec::new();
        for result in &mut results {
            if result.status == common::ToolStatus::Ok {
                let reasons = crate::agents::untrusted_input::UntrustedInputProcessor
                    ::sanitize_tool_result_data(result, 0.8);
                rejected.extend(reasons);
            }
        }
        if !rejected.is_empty() {
            let _ = ctx
                .sink
                .emit(AgentEvent::DebugTrace {
                    kind: "untrusted_input.rejected".to_string(),
                    payload: serde_json::json!({
                        "source": "chat.atomic_tools",
                        "rejected_count": rejected.len(),
                    }),
                })
                .await;
        }

        // Record tool call details for white-box reporting.
        for (call, result) in calls.iter().zip(results.iter()) {
            let elapsed_ms = result.trace.as_ref().and_then(|t| t.elapsed_ms).unwrap_or(0);
            ctx.tool_call_records.push(crate::agents::runtime::ToolCallRecord {
                tool: call.tool.clone(),
                iteration: 0,
                args: call.args.clone(),
                status: result.status,
                elapsed_ms,
            });
            let _ = ctx
                .sink
                .emit(AgentEvent::ToolResult {
                    tool: call.tool.clone(),
                    status: result.status,
                    data: result.data.clone(),
                    elapsed_ms,
                })
                .await;
        }

        ctx.tool_results.extend(results.iter().cloned());

        Ok(StepOutcome::Next(Box::new(ChatState::Answer)))
    }

    // --- Answer step ---

    async fn step_answer(&self, ctx: &mut ChatContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        // Build system prompt from answer skill.
        let system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
            crate::agents::strategy::prompts::chat::ANSWER_SKILL_ID,
            "chat",
            &[],
        );

        let mut messages = build_chat_messages_with_system(&ctx.request, &system_prompt);

        // Inject atomic tool results so the answer can reference them.
        if !ctx.tool_results.is_empty() {
            let mut context = String::from("Tool results:\n");
            for result in &ctx.tool_results {
                context.push_str(&format!("\n### {}\n", result.tool));
                if result.status == ToolStatus::Ok {
                    if let Some(data) = &result.data {
                        context.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                } else if let Some(data) = &result.data
                    && let Some(error) = data.get("error").and_then(|v| v.as_str())
                {
                    context.push_str(&format!("Error: {error}"));
                }
            }
            messages.push(avrag_llm::ChatMessage::user(context));
        }

        let sink = ctx.sink.as_ref();
        let cancel = ctx.cancel.clone();

        let response = if ctx.request.stream {
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let llm_client = self.llm_client.as_ref()
                .ok_or_else(|| AgentErrorKind::ModelUnavailable {
                    provider: "unknown".to_string(),
                    model: "streaming requires LlmClient".to_string(),
                })?;
            let stream = llm_client.complete_stream(&messages, self.temperature, cancel, move |delta| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            });
            tokio::pin!(stream);

            let response = loop {
                tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => {
                        return Err(AgentErrorKind::Unknown("cancelled".to_string()));
                    }
                    delta = delta_rx.recv() => {
                        if let Some(delta) = delta {
                            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                        }
                    }
                    result = &mut stream => {
                        break result.map_err(|_e| AgentErrorKind::ModelUnavailable {
                            provider: "unknown".to_string(),
                            model: "unknown".to_string(),
                        })?;
                    }
                }
            };

            while let Ok(delta) = delta_rx.try_recv() {
                let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
            }

            response
        } else {
            let response = tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => {
                    return Err(AgentErrorKind::Unknown("cancelled".to_string()));
                }
                result = self.llm.complete(&messages, self.temperature) => {
                    result.map_err(|_e| AgentErrorKind::ModelUnavailable {
                        provider: "unknown".to_string(),
                        model: "unknown".to_string(),
                    })?
                }
            };
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: response.content.clone(),
                })
                .await;
            response
        };

        // Accumulate usage.
        ctx.request_count += 1;
        ctx.aggregated_usage = Some(crate::agents::unified::helpers::merge_usage(
            ctx.aggregated_usage.as_ref(),
            &response.usage,
        ));

        let run_usage = crate::agents::unified::helpers::build_run_usage(
            ctx.aggregated_usage.as_ref(),
            ctx.request_count,
        );

        let _ = crate::agents::unified::helpers::emit_usage(sink, run_usage.as_ref()).await;

        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(response.content.clone()),
                usage: run_usage.as_ref().map(crate::agents::unified::helpers::run_usage_to_agent_usage),
            })
            .await;

        let mut result = AgentRunResult {
            answer: response.content,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.tool_results),
            final_decision: Some(FinalDecision::Synthesized),
            ..Default::default()
        };

        // White-box: record plan decision if available.
        if let Some(ref action) = ctx.plan_decision_action {
            let tool_count = result.tool_calls.len();
            let reasoning = if tool_count > 0 {
                format!("plan selected {} tool(s) for execution", tool_count)
            } else {
                "plan decided to answer directly without tools".to_string()
            };
            result.decisions.push(crate::agents::runtime::DecisionRecord {
                phase: "plan".to_string(),
                iteration: 0,
                decision: action.clone(),
                reasoning,
                selected_tools: ctx.tool_call_records.iter().map(|r| r.tool.clone()).collect(),
            });
        }
        result.tool_calls = std::mem::take(&mut ctx.tool_call_records);
        result.degrade_trace.extend(std::mem::take(&mut ctx.content_guard_trace));

        Ok(StepOutcome::Terminate(result))
    }
}

// ---------------------------------------------------------------------------
// Plan result (bridge between v4 and v5)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Helpers (migrated from mode_chat.rs)
// ---------------------------------------------------------------------------

fn build_chat_messages_with_system(
    request: &AgentRequest,
    system_prompt: &str,
) -> Vec<avrag_llm::ChatMessage> {
    let mut system = String::from(system_prompt);
    if let Some(summary) = request
        .session_summary
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        system.push_str("\n\nSession summary:\n");
        system.push_str(summary.trim());
    }
    if let Some(preferences) = request.user_preferences.as_ref() {
        system.push_str("\n\nUser preferences:\n");
        system.push_str(&preferences.to_string());
    }

    let mut messages = vec![avrag_llm::ChatMessage::system(system)];
    for message in &request.messages {
        match message.role.as_str() {
            "assistant" => messages.push(avrag_llm::ChatMessage::assistant(&message.content,
            )),
            _ => messages.push(avrag_llm::ChatMessage::user(&message.content,
            )),
        }
    }
    messages.push(avrag_llm::ChatMessage::user(&request.query,
    ));
    messages
}

#[derive(Debug, Default)]
struct ChatPlanDecision {
    action: String,
    clarification_message: String,
    calls: Vec<ToolCall>,
}

fn parse_chat_plan_decision(raw: &str) -> ChatPlanDecision {
    let start = raw.find('{');
    let end = raw.rfind('}');
    let json_str = match (start, end) {
        (Some(s), Some(e)) if s <= e => &raw[s..=e],
        _ => raw.trim(),
    };

    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => {
            return ChatPlanDecision {
                action: "answer".to_string(),
                ..ChatPlanDecision::default()
            }
        }
    };

    let action = value
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("answer")
        .to_string();

    let clarification_message = value
        .get("clarification_message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let calls: Vec<ToolCall> = value
        .get("calls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let tool = item.get("tool")?.as_str()?;
                    let args = item.get("args").cloned().unwrap_or(serde_json::json!({}));
                    Some(ToolCall {
                        tool: tool.to_string(),
                        version: "1.0".to_string(),
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    ChatPlanDecision {
        action,
        clarification_message,
        calls,
    }
}

/// Result of the Chat Plan phase.
pub enum ChatPlanResult {
    /// Ask the user for clarification.
    Clarify(String),
    /// No tools needed — answer directly.
    AnswerDirectly,
    /// Execute atomic tools before answering.
    UseTools(Vec<ToolCall>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_state_ids() {
        assert_eq!(ChatState::Plan.state_id(), "plan");
        assert_eq!(
            ChatState::ExecuteAtomic { calls: vec![] }.state_id(),
            "execute_atomic"
        );
        assert_eq!(ChatState::Answer.state_id(), "answer");
    }

    #[test]
    fn chat_state_kinds() {
        assert_eq!(ChatState::Plan.state_kind(), StateKind::Plan);
        assert_eq!(
            ChatState::ExecuteAtomic { calls: vec![] }.state_kind(),
            StateKind::Execute
        );
        assert_eq!(ChatState::Answer.state_kind(), StateKind::Answer);
    }

    #[test]
    fn parse_chat_plan_decision_answer() {
        let raw = r#"{"action": "answer", "calls": []}"#;
        let decision = parse_chat_plan_decision(raw);
        assert_eq!(decision.action, "answer");
        assert!(decision.calls.is_empty());
    }

    #[test]
    fn parse_chat_plan_decision_clarify() {
        let raw = r#"{"action": "clarify", "clarification_message": "which dataset?"}"#;
        let decision = parse_chat_plan_decision(raw);
        assert_eq!(decision.action, "clarify");
        assert_eq!(decision.clarification_message, "which dataset?");
    }

    #[test]
    fn parse_chat_plan_decision_with_tools() {
        let raw = r#"{"action": "answer", "calls": [{"tool": "calculator", "args": {"expression": "1+1"}}]}"#;
        let decision = parse_chat_plan_decision(raw);
        assert_eq!(decision.action, "answer");
        assert_eq!(decision.calls.len(), 1);
        assert_eq!(decision.calls[0].tool, "calculator");
    }

    #[test]
    fn parse_chat_plan_decision_invalid_json_defaults_to_answer() {
        let raw = "not json";
        let decision = parse_chat_plan_decision(raw);
        assert_eq!(decision.action, "answer");
        assert!(decision.calls.is_empty());
    }
}
