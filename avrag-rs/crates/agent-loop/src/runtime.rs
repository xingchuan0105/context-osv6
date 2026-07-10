use crate::AgentKind;
use crate::events::AgentEventSink;
use crate::react_loop::DegradeReason;
use contracts::auth_runtime::{AuthContext, UserId, SubjectKind};
use contracts::chat::{ChatTurnInput, Citation, DegradeTraceItem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Stub auth for offline tests / eval / redteam when no real principal is available.
pub fn stub_agent_auth() -> AuthContext {
    AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::System)
}

/// Layer-3 user profile carried into the agent loop (not UI `UserPreferences`).
///
/// Built from chatmemory `Layer3Profile` at request assembly; open extension
/// stays in `custom_preferences` / `structured_profile` JSON objects only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentUserPreferences {
    #[serde(default)]
    pub expertise_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_answer_style: Option<String>,
    #[serde(default)]
    pub frequently_asked_topics: Vec<String>,
    #[serde(default)]
    pub custom_preferences: serde_json::Value,
    #[serde(default)]
    pub structured_profile: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_version: Option<String>,
}

impl AgentUserPreferences {
    pub fn from_layer3(profile: &avrag_chatmemory::Layer3Profile) -> Self {
        Self {
            expertise_domains: profile.expertise_domains.clone(),
            preferred_answer_style: profile.preferred_answer_style.clone(),
            frequently_asked_topics: profile.frequently_asked_topics.clone(),
            custom_preferences: profile.custom_preferences.clone(),
            structured_profile: profile.structured_profile.clone(),
            inference_version: Some(profile.inference_version.clone()).filter(|s| !s.is_empty()),
        }
    }
}

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

/// Guaranteed recent user turns injected unconditionally (memory floor): current query + 2 prior.
pub use agent_tools::MAX_PROMPT_HISTORY_TURNS;

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
    /// Workspace / workspace context.
    pub workspace_id: Option<String>,
    /// Session ID for continuity.
    pub session_id: Option<String>,
    /// Document scope for RAG (server-controlled, never expanded by model).
    pub doc_scope: Vec<String>,
    /// Recent message history.
    pub messages: Vec<ChatTurnInput>,
    /// Layer-3 user profile memory (strong-typed; not UI dashboard prefs).
    pub user_preferences: Option<AgentUserPreferences>,
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
    /// Strong-typed auth / org context (TN Wave 4 — no more JSON Value boundary collapse).
    pub auth: AuthContext,
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
            .field("workspace_id", &self.workspace_id)
            .field("session_id", &self.session_id)
            .field("doc_scope", &self.doc_scope)
            .field("messages", &self.messages)
            .field("user_preferences", &self.user_preferences)
            .field("debug", &self.debug)
            .field("stream", &self.stream)
            .field("language", &self.language)
            .field("auth", &self.auth)
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
    /// Routing decision that selected this strategy (white-box observability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_decision: Option<String>,
}

/// Budget consumption at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetUsage {
    pub current: u8,
    pub max: u8,
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
    use crate::events::{AgentEvent, CollectingSink};

    #[test]
    fn test_agent_request_serde_roundtrip() {
        let req = AgentRequest {
            kind: AgentKind::Chat,
            query: "hello".to_string(),
            workspace_id: Some("nb-1".to_string()),
            session_id: Some("sess-1".to_string()),
            doc_scope: vec!["doc-1".to_string()],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth: AuthContext::new(
                UserId::from(Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").unwrap()),
                SubjectKind::User,
            ),
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
}
