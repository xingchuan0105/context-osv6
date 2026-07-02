use super::AgentKind;
use crate::agents::events::AgentEventSink;
use crate::agents::react_loop::DegradeReason;
use contracts::chat::{ChatTurnInput, Citation, DegradeTraceItem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Objective signals computed from a single iteration's results.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSignals {
    /// Total number of distinct results returned from this iteration.
    pub recall_count: usize,
    /// Highest retrieval score across results (0.0 if no results).
    pub max_score: f32,
    /// Fraction of significant query terms that appear in at least one hit.
    /// Range: 0.0 (no overlap) to 1.0 (every term covered).
    pub term_coverage: f32,
    /// Subqueries that returned zero hits — useful for targeted broaden/replan.
    pub zero_hits_per_subquery: Vec<String>,
}

impl EvaluationSignals {
    /// Compute term coverage from a query and a list of result text snippets.
    /// Lower-cases both sides; counts a term as covered if any snippet contains it.
    /// Filters short stop-tokens (length < 3) so coverage isn't dominated by
    /// articles and conjunctions.
    pub fn compute_term_coverage(query: &str, result_texts: &[&str]) -> f32 {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| t.chars().count() >= 3)
            .collect();
        if terms.is_empty() {
            return 1.0; // degenerate case — treat as fully covered.
        }
        let blob: String = result_texts
            .iter()
            .map(|t| t.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let covered = terms.iter().filter(|t| blob.contains(t.as_str())).count();
        covered as f32 / terms.len() as f32
    }
}

// ---------------------------------------------------------------------------
// TraceSpan — distributed tracing structures
// ---------------------------------------------------------------------------

/// A single span in a distributed trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub name: String,
    pub started_at_ms: u64,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TraceSpan {
    /// Create a new span.
    pub fn new(trace_id: &str, name: &str, parent_id: Option<&str>) -> Self {
        Self {
            id: format!("{}-{}", trace_id, uuid::Uuid::new_v4()),
            parent_id: parent_id.map(|s| s.to_string()),
            name: name.to_string(),
            started_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            elapsed_ms: 0,
            attributes: BTreeMap::new(),
            error: None,
        }
    }

    /// Set an attribute on the span.
    pub fn set_attribute<K: Into<String>, V: Into<serde_json::Value>>(&mut self, key: K, value: V) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Mark the span as finished, computing elapsed time.
    pub fn finish(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.elapsed_ms = now.saturating_sub(self.started_at_ms);
    }

    /// Mark the span as failed with an error message and finish it.
    pub fn set_error(&mut self, error: &str) {
        self.error = Some(error.to_string());
        self.finish();
    }
}

/// Full distributed trace for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTrace {
    pub trace_id: String,
    pub spans: Vec<TraceSpan>,
    pub total_elapsed_ms: u64,
    pub budget_used: u8,
}

impl AgentTrace {
    pub fn new(trace_id: &str) -> Self {
        Self {
            trace_id: trace_id.to_string(),
            spans: Vec::new(),
            total_elapsed_ms: 0,
            budget_used: 0,
        }
    }

    /// Add a span to the trace.
    pub fn add_span(&mut self, mut span: TraceSpan) {
        if span.elapsed_ms == 0 {
            span.finish();
        }
        self.spans.push(span);
    }

    /// Find a span by name.
    pub fn find_span(&self, name: &str) -> Option<&TraceSpan> {
        self.spans.iter().find(|s| s.name == name)
    }

    /// Return all direct children of a given span id.
    pub fn children_of(&self, parent_id: &str) -> Vec<&TraceSpan> {
        self.spans
            .iter()
            .filter(|s| s.parent_id.as_deref() == Some(parent_id))
            .collect()
    }
}

/// Guaranteed recent user turns injected unconditionally (memory floor): current query + 2 prior.
pub const MAX_PROMPT_HISTORY_TURNS: usize = 2;

/// Return the most recent `max_turns` messages from the history slice.
pub fn recent_messages(messages: &[ChatTurnInput], max_turns: usize) -> &[ChatTurnInput] {
    let start = messages.len().saturating_sub(max_turns);
    &messages[start..]
}

/// Request context passed to any agent implementation.
/// Concrete agents may downcast or extend this with agent-specific fields.
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Canonical agent kind (already normalized from user input).
    pub kind: AgentKind,
    /// User's natural language query. Also used as retrieval/fallback query
    /// (ADR-0010: server-side query normalization removed; LLM resolves anaphora
    /// on its own via the memory cluster).
    pub query: String,
    /// Notebook / workspace context.
    pub notebook_id: Option<String>,
    /// Session ID for continuity.
    pub session_id: Option<String>,
    /// Document scope for RAG (server-controlled, never expanded by model).
    pub doc_scope: Vec<String>,
    /// Recent message history.
    pub messages: Vec<ChatTurnInput>,
    /// User preference memory (layer-3 memory).
    pub user_preferences: Option<serde_json::Value>,
    /// Debug flag: when true, agents emit DebugTrace events.
    pub debug: bool,
    /// Stream flag: when true, the caller expects incremental events via sink.
    pub stream: bool,
    /// Preferred language for agent output (e.g. "zh", "en").
    pub language: Option<String>,
    /// Optional: override Plan-phase tool selection (intervention).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred_tools: Vec<String>,
    /// Optional: hint for answer format skill (intervention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_hint: Option<String>,
    /// Optional: override max iteration budget (intervention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u8>,
    /// Auth / org context serialized as JSON to avoid leaking auth types into agent layer.
    pub auth_context: serde_json::Value,
    /// Docscope metadata for RAG (loaded by orchestrator, passed to agent).
    #[serde(default)]
    pub docscope_metadata: Option<common::DocScopeMetadata>,
    /// Free-form metadata (e.g. source_type, source_token for shared KB).
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    /// Cancellation token from the orchestrator. When triggered, agents must
    /// abort in-flight LLM streams and return promptly.
    /// Not serialized — runtime-only field.
    #[serde(skip)]
    pub cancellation_token: Option<CancellationToken>,
    /// Optional guard pipeline for content sanitization within agents.
    /// Not serialized — runtime-only field.
    #[serde(skip)]
    pub guard_pipeline: Option<Arc<avrag_guardrails::GuardPipeline>>,
}

impl AgentRequest {}

impl std::fmt::Debug for AgentRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRequest")
            .field("kind", &self.kind)
            .field("query", &self.query)
            .field("notebook_id", &self.notebook_id)
            .field("session_id", &self.session_id)
            .field("doc_scope", &self.doc_scope)
            .field("messages", &self.messages)
            .field("user_preferences", &self.user_preferences)
            .field("debug", &self.debug)
            .field("stream", &self.stream)
            .field("language", &self.language)
            .field("auth_context", &self.auth_context)
            .field("docscope_metadata", &self.docscope_metadata)
            .field("metadata", &self.metadata)
            .field("cancellation_token", &self.cancellation_token.is_some())
            .field("guard_pipeline", &self.guard_pipeline.is_some())
            .finish()
    }
}

/// Result of a completed agent run.
/// Streaming paths accumulate this from sink events; non-streaming paths return it directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRunResult {
    pub answer: String,
    #[serde(default)]
    pub answer_blocks: Vec<contracts::chat::AnswerBlock>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub sources: Vec<contracts::chat::SourceRef>,
    #[serde(default)]
    pub reasoning_summary: Option<String>,
    #[serde(default)]
    pub degrade_trace: Vec<DegradeTraceItem>,
    #[serde(default)]
    pub usage: Option<AgentRunUsage>,
    #[serde(default)]
    pub debug_payload: Option<serde_json::Value>,
    #[serde(default)]
    pub message_id: Option<i64>,
    /// Per-iteration trace from ReAct agents (RAG/Search). Empty for legacy
    /// single-shot agents (Chat).
    #[serde(default)]
    pub iterations: Vec<IterationRecord>,
    /// Total tool calls accumulated across all iterations.
    #[serde(default)]
    pub total_tool_calls: u32,
    /// Atomic tool results accumulated during the run (available in all modes).
    #[serde(default)]
    pub tool_results: Vec<contracts::ToolResult>,
    /// Terminal decision of the loop. `None` for legacy single-shot agents.
    #[serde(default)]
    pub final_decision: Option<FinalDecision>,

    // ===== v5 white-box fields (all serde(default) for backward compat) =====
    /// Trace ID for distributed tracing.
    #[serde(default)]
    pub trace_id: Option<String>,
    /// Budget consumption snapshot at loop termination.
    #[serde(default)]
    pub budget_used: Option<BudgetUsage>,
    /// Total wall-clock time from init to termination, in milliseconds.
    #[serde(default)]
    pub total_elapsed_ms: Option<u64>,
    /// Full distributed trace with spans (debug mode or when tracing is enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<AgentTrace>,
    /// Replay snapshot for debugging/evaluation (opt-in, may be large).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<crate::agents::replay::ReplaySnapshot>,
    /// Planning / evaluation decisions recorded during the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<DecisionRecord>,
    /// Tool invocation records collected during the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallRecord>,
    /// Routing decision that selected this strategy (white-box observability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_decision: Option<String>,
    /// Evaluation summary synthesized across all iterations (white-box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_summary: Option<String>,
}

/// Budget consumption at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetUsage {
    pub current: u8,
    pub max: u8,
}

/// Record of a planning or evaluation decision made during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Which phase produced the decision: "plan", "evaluate", "decompose".
    pub phase: String,
    /// Iteration number (0-indexed). For single-phase strategies this is 0.
    pub iteration: u8,
    /// The decision taken: e.g. "synthesize", "replan", "clarify", "broaden_query".
    pub decision: String,
    /// Reasoning or explanation for the decision.
    pub reasoning: String,
    /// Tools selected as part of this decision (if any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_tools: Vec<String>,
}

/// Record of a single tool invocation during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool identifier.
    pub tool: String,
    /// Iteration number (0-indexed).
    pub iteration: u8,
    /// Arguments passed to the tool.
    pub args: serde_json::Value,
    /// Execution status.
    pub status: contracts::ToolStatus,
    /// Wall-clock time of the call, in milliseconds.
    pub elapsed_ms: u64,
}

/// Per-iteration trace recorded by ReAct loops (RAG/Search).
///
/// One record is appended per iteration *after* the evaluator has produced its
/// advice, so the `decision` field reflects what the loop did next (continue
/// with which branch, or terminate with synthesize/clarify/degrade).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    /// 0-indexed iteration number within this agent run.
    pub iteration: u8,
    /// Plan / params snapshot captured at the start of the iteration
    /// (schema-agnostic so each agent can record its own param shape).
    pub plan: serde_json::Value,
    /// Objective signals computed from this iteration's results.
    pub signals: EvaluationSignals,
    /// Decision taken after evaluating signals — one of the stable identifier
    /// strings: `synthesize` / `clarify` / `degrade` / `replan` /
    /// `broaden_query` / `escalate_vertical` / `escalate_to_search` /
    /// `fetch_full_page`.
    pub decision: String,
    /// Wall-clock time spent on this iteration, in milliseconds.
    pub elapsed_ms: u64,
    /// Structured output from the LLM strategy evaluator, when one was run.
    #[serde(default)]
    pub llm_evaluation: Option<serde_json::Value>,
    /// Per-iteration token usage (planner + evaluator + any provider calls).
    #[serde(default)]
    pub usage: Option<AgentRunUsage>,
}

/// Terminal outcome of a ReAct loop. `None` on `AgentRunResult.final_decision`
/// signals a legacy single-shot agent that does not iterate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FinalDecision {
    /// Loop ended by handing accumulated context to the synthesizer.
    Synthesized,
    /// Loop ended with a direct assistant answer, skipping synthesis (chat mode).
    DirectAnswer,
    /// Loop ended by asking the user a clarifying question.
    Clarified { question: String },
    /// Loop ended by emitting a degrade trace and returning a partial answer.
    Degraded { reason: DegradeReason },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunUsage {
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub request_count: u64,
    #[serde(default)]
    pub cached_tokens: u64,
}

/// Trait for concrete agent implementations.
/// Each agent (Chat, WebSearch, Rag) implements this trait.
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Run the agent, emitting events via the provided sink.
    /// The returned `AgentRunResult` must match the final state represented by sink events.
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, common::AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::{AgentEvent, CollectingSink};

    #[test]
    fn test_agent_request_serde_roundtrip() {
        let req = AgentRequest {
            kind: AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: Some("nb-1".to_string()),
            session_id: Some("sess-1".to_string()),
            doc_scope: vec!["doc-1".to_string()],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({"org_id": "o1"}),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: AgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.kind, parsed.kind);
        assert_eq!(req.query, parsed.query);
    }

    #[test]
    fn test_agent_run_result_default() {
        let result = AgentRunResult::default();
        assert!(result.answer.is_empty());
        assert!(result.citations.is_empty());
        assert!(result.degrade_trace.is_empty());
        assert!(result.reasoning_summary.is_none());
        // New ReAct-extension fields default to empty / zero / none.
        assert!(result.iterations.is_empty());
        assert_eq!(result.total_tool_calls, 0);
        assert!(result.final_decision.is_none());
    }

    #[test]
    fn test_agent_run_result_legacy_json_deserializes_via_serde_default() {
        // Old call sites (Chat agent, contract tests) emit JSON without the
        // ReAct fields. `#[serde(default)]` must keep deserialisation green so
        // we can ship the schema extension without coordinating consumer rollout.
        let legacy_json = r#"{
            "answer": "hello",
            "answer_blocks": [],
            "citations": [],
            "sources": [],
            "reasoning_summary": null,
            "degrade_trace": [],
            "usage": null,
            "debug_payload": null,
            "message_id": null
        }"#;
        let parsed: AgentRunResult = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.answer, "hello");
        assert!(parsed.iterations.is_empty());
        assert_eq!(parsed.total_tool_calls, 0);
        assert!(parsed.final_decision.is_none());
    }

    #[test]
    fn test_iteration_record_serde_roundtrip() {
        let record = IterationRecord {
            iteration: 1,
            plan: serde_json::json!({"queries": ["a", "b"]}),
            signals: EvaluationSignals {
                recall_count: 4,
                max_score: 0.42,
                term_coverage: 0.6,
                zero_hits_per_subquery: vec!["a".to_string()],
            },
            decision: "broaden_query".to_string(),
            elapsed_ms: 137,
            llm_evaluation: None,
            usage: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: IterationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.iteration, 1);
        assert_eq!(parsed.signals.recall_count, 4);
        assert_eq!(parsed.decision, "broaden_query");
        assert_eq!(parsed.elapsed_ms, 137);
    }

    #[test]
    fn test_final_decision_synthesized_serde_tagged() {
        let json = serde_json::to_string(&FinalDecision::Synthesized).unwrap();
        // Tag form, snake_case — telemetry/UI agreement.
        assert_eq!(json, r#"{"kind":"synthesized"}"#);
        let parsed: FinalDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, FinalDecision::Synthesized);
    }

    #[test]
    fn test_final_decision_direct_answer_serde_tagged() {
        let json = serde_json::to_string(&FinalDecision::DirectAnswer).unwrap();
        assert_eq!(json, r#"{"kind":"direct_answer"}"#);
        let parsed: FinalDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, FinalDecision::DirectAnswer);
    }

    #[test]
    fn test_final_decision_clarified_carries_question() {
        let decision = FinalDecision::Clarified {
            question: "which dataset?".to_string(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: FinalDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, decision);
    }

    #[test]
    fn test_final_decision_degraded_serializes_reason() {
        let decision = FinalDecision::Degraded {
            reason: DegradeReason::BudgetExhausted,
        };
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: FinalDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, decision);
    }

    #[test]
    fn test_agent_run_result_with_iterations_roundtrip() {
        let mut result = AgentRunResult::default();
        result.iterations.push(IterationRecord {
            iteration: 0,
            plan: serde_json::json!({"q": "rust async"}),
            signals: EvaluationSignals::default(),
            decision: "synthesize".to_string(),
            elapsed_ms: 250,
            llm_evaluation: None,
            usage: None,
        });
        result.total_tool_calls = 2;
        result.final_decision = Some(FinalDecision::Synthesized);

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AgentRunResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.iterations.len(), 1);
        assert_eq!(parsed.iterations[0].decision, "synthesize");
        assert_eq!(parsed.total_tool_calls, 2);
        assert_eq!(parsed.final_decision, Some(FinalDecision::Synthesized));
    }

    #[test]
    fn recent_messages_returns_last_n_items() {
        let messages: Vec<ChatTurnInput> = (0..12)
            .map(|i| ChatTurnInput {
                role: if i % 2 == 0 {
                    "user".to_string()
                } else {
                    "assistant".to_string()
                },
                content: format!("msg-{i}"),
                resolved_query: None,
            })
            .collect();
        let recent = super::recent_messages(&messages, super::MAX_PROMPT_HISTORY_TURNS);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "msg-10");
        assert_eq!(recent[1].content, "msg-11");
    }

    #[test]
    fn recent_messages_returns_all_when_under_limit() {
        let messages: Vec<ChatTurnInput> = (0..2)
            .map(|i| ChatTurnInput {
                role: "user".to_string(),
                content: format!("msg-{i}"),
                resolved_query: None,
            })
            .collect();
        let recent = super::recent_messages(&messages, super::MAX_PROMPT_HISTORY_TURNS);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "msg-0");
    }

    #[test]
    fn recent_messages_handles_empty() {
        let messages: Vec<ChatTurnInput> = vec![];
        let recent = super::recent_messages(&messages, super::MAX_PROMPT_HISTORY_TURNS);
        assert!(recent.is_empty());
    }

    #[tokio::test]
    async fn test_collecting_sink_in_runtime_context() {
        let sink = CollectingSink::new();
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: "Hello".to_string(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some("Hello".to_string()),
                usage: None,
            })
            .await;
        let events = sink.into_events();
        assert_eq!(events.len(), 2);
    }

    // -----------------------------------------------------------------------
    // TraceSpan tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_trace_span_new_has_id_and_name() {
        let span = TraceSpan::new("trace-1", "agent.run", None);
        assert!(span.id.starts_with("trace-1-"));
        assert_eq!(span.name, "agent.run");
        assert!(span.parent_id.is_none());
        assert!(span.error.is_none());
    }

    #[test]
    fn test_trace_span_with_parent() {
        let parent = TraceSpan::new("trace-1", "agent.run", None);
        let child = TraceSpan::new("trace-1", "state.plan", Some(&parent.id));
        assert_eq!(child.parent_id, Some(parent.id));
    }

    #[test]
    fn test_trace_span_set_attribute() {
        let mut span = TraceSpan::new("t", "agent.run", None);
        span.set_attribute("strategy", "ChatStrategy");
        span.set_attribute("budget", 3u8);
        assert_eq!(
            span.attributes.get("strategy"),
            Some(&serde_json::Value::String("ChatStrategy".to_string()))
        );
        assert_eq!(
            span.attributes.get("budget"),
            Some(&serde_json::Value::Number(3.into()))
        );
    }

    #[test]
    fn test_trace_span_finish_computes_elapsed() {
        let mut span = TraceSpan::new("t", "agent.run", None);
        std::thread::sleep(std::time::Duration::from_millis(10));
        span.finish();
        assert!(
            span.elapsed_ms >= 10,
            "elapsed_ms should be >= 10, got {}",
            span.elapsed_ms
        );
    }

    #[test]
    fn test_trace_span_set_error_sets_error_and_finishes() {
        let mut span = TraceSpan::new("t", "agent.run", None);
        span.set_error("something went wrong");
        assert_eq!(span.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_trace_span_serde_roundtrip() {
        let mut span = TraceSpan::new("t", "agent.run", None);
        span.set_attribute("key", "value");
        span.finish();

        let json = serde_json::to_string(&span).unwrap();
        let parsed: TraceSpan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "agent.run");
        assert_eq!(
            parsed.attributes.get("key"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn test_trace_span_serde_omits_empty_fields() {
        let span = TraceSpan::new("t", "agent.run", None);
        let json = serde_json::to_string(&span).unwrap();
        assert!(!json.contains("error"));
        assert!(!json.contains("parent_id"));
    }

    // -----------------------------------------------------------------------
    // AgentTrace tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_agent_trace_new() {
        let trace = AgentTrace::new("trace-1");
        assert_eq!(trace.trace_id, "trace-1");
        assert!(trace.spans.is_empty());
        assert_eq!(trace.total_elapsed_ms, 0);
        assert_eq!(trace.budget_used, 0);
    }

    #[test]
    fn test_agent_trace_add_span() {
        let mut trace = AgentTrace::new("trace-1");
        let span = TraceSpan::new("trace-1", "agent.run", None);
        trace.add_span(span);
        assert_eq!(trace.spans.len(), 1);
        assert_eq!(trace.spans[0].name, "agent.run");
    }

    #[test]
    fn test_agent_trace_find_span() {
        let mut trace = AgentTrace::new("trace-1");
        trace.add_span(TraceSpan::new("trace-1", "agent.run", None));
        trace.add_span(TraceSpan::new("trace-1", "state.plan", None));

        let found = trace.find_span("state.plan");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "state.plan");

        assert!(trace.find_span("nonexistent").is_none());
    }

    #[test]
    fn test_agent_trace_children_of() {
        let mut trace = AgentTrace::new("trace-1");
        let root = TraceSpan::new("trace-1", "agent.run", None);
        let child1 = TraceSpan::new("trace-1", "state.plan", Some(&root.id));
        let child2 = TraceSpan::new("trace-1", "state.execute", Some(&root.id));
        let orphan = TraceSpan::new("trace-1", "other", None);

        trace.add_span(root);
        trace.add_span(child1);
        trace.add_span(child2);
        trace.add_span(orphan);

        let root_id = trace.find_span("agent.run").unwrap().id.clone();
        let children = trace.children_of(&root_id);
        assert_eq!(children.len(), 2);
        assert!(children.iter().any(|s| s.name == "state.plan"));
        assert!(children.iter().any(|s| s.name == "state.execute"));
    }

    #[test]
    fn test_agent_trace_serde_roundtrip() {
        let mut trace = AgentTrace::new("trace-1");
        trace.total_elapsed_ms = 150;
        trace.budget_used = 2;
        let mut span = TraceSpan::new("trace-1", "agent.run", None);
        span.finish();
        trace.add_span(span);

        let json = serde_json::to_string(&trace).unwrap();
        let parsed: AgentTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trace_id, "trace-1");
        assert_eq!(parsed.total_elapsed_ms, 150);
        assert_eq!(parsed.budget_used, 2);
        assert_eq!(parsed.spans.len(), 1);
    }

    #[test]
    fn test_agent_run_result_with_trace_roundtrip() {
        let mut result = AgentRunResult::default();
        result.trace_id = Some("trace-1".to_string());

        let mut trace = AgentTrace::new("trace-1");
        let mut root = TraceSpan::new("trace-1", "agent.run", None);
        root.set_attribute("strategy", "ChatStrategy");
        root.finish();
        trace.add_span(root);
        trace.total_elapsed_ms = 200;
        result.trace = Some(trace);

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AgentRunResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.trace_id, Some("trace-1".to_string()));
        let t = parsed.trace.unwrap();
        assert_eq!(t.trace_id, "trace-1");
        assert_eq!(t.spans.len(), 1);
        assert_eq!(t.spans[0].name, "agent.run");
    }

    #[test]
    fn test_agent_run_result_with_decisions_and_tool_calls() {
        let mut result = AgentRunResult::default();
        result.decisions.push(DecisionRecord {
            phase: "plan".to_string(),
            iteration: 0,
            decision: "synthesize".to_string(),
            reasoning: "test".to_string(),
            selected_tools: vec!["dense_retrieval".to_string()],
        });
        result.tool_calls.push(ToolCallRecord {
            tool: "dense_retrieval".to_string(),
            iteration: 0,
            args: serde_json::json!({"query": "rust"}),
            status: contracts::ToolStatus::Ok,
            elapsed_ms: 42,
        });

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AgentRunResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.decisions[0].phase, "plan");
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].tool, "dense_retrieval");
        assert_eq!(parsed.tool_calls[0].elapsed_ms, 42);
    }

    #[test]
    fn test_decision_record_serde_roundtrip() {
        let record = DecisionRecord {
            phase: "evaluate".to_string(),
            iteration: 1,
            decision: "replan".to_string(),
            reasoning: "low recall".to_string(),
            selected_tools: vec!["web_search".to_string()],
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: DecisionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.phase, "evaluate");
        assert_eq!(parsed.iteration, 1);
        assert_eq!(parsed.selected_tools, vec!["web_search"]);
    }

    #[test]
    fn test_tool_call_record_serde_roundtrip() {
        let record = ToolCallRecord {
            tool: "calculator".to_string(),
            iteration: 0,
            args: serde_json::json!({"expr": "1+1"}),
            status: contracts::ToolStatus::Ok,
            elapsed_ms: 15,
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: ToolCallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool, "calculator");
        assert!(matches!(parsed.status, contracts::ToolStatus::Ok));
    }

    #[test]
    fn test_legacy_json_without_new_fields_deserializes() {
        // Ensure backward compat: old JSON without decisions/tool_calls fields
        let legacy_json = r#"{
            "answer": "hello",
            "answer_blocks": [],
            "citations": [],
            "sources": [],
            "reasoning_summary": null,
            "degrade_trace": [],
            "usage": null,
            "debug_payload": null,
            "message_id": null,
            "iterations": [],
            "total_tool_calls": 0,
            "tool_results": [],
            "final_decision": null,
            "trace_id": null,
            "budget_used": null,
            "total_elapsed_ms": null,
            "trace": null,
            "snapshot": null
        }"#;
        let parsed: AgentRunResult = serde_json::from_str(legacy_json).unwrap();
        assert!(parsed.decisions.is_empty());
        assert!(parsed.tool_calls.is_empty());
    }
}
