use super::AgentKind;
use crate::agents::evaluator::EvaluationSignals;
use crate::agents::events::AgentEventSink;
use crate::agents::react_loop::DegradeReason;
use common::{ChatTurnInput, Citation, DegradeTraceItem};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

/// Request context passed to any agent implementation.
/// Concrete agents may downcast or extend this with agent-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// Canonical agent kind (already normalized from user input).
    pub kind: AgentKind,
    /// User's natural language query.
    pub query: String,
    /// Notebook / workspace context.
    pub notebook_id: Option<String>,
    /// Session ID for continuity.
    pub session_id: Option<String>,
    /// Document scope for RAG (server-controlled, never expanded by model).
    pub doc_scope: Vec<String>,
    /// Recent message history.
    pub messages: Vec<ChatTurnInput>,
    /// Optional session summary (layer-2 memory).
    pub session_summary: Option<String>,
    /// User preference memory (layer-3 memory).
    pub user_preferences: Option<serde_json::Value>,
    /// Debug flag: when true, agents emit DebugTrace events.
    pub debug: bool,
    /// Stream flag: when true, the caller expects incremental events via sink.
    pub stream: bool,
    /// Preferred language for agent output (e.g. "zh", "en").
    pub language: Option<String>,
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
    pub sources: Vec<common::SourceRef>,
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
    /// Terminal decision of the loop. `None` for legacy single-shot agents.
    #[serde(default)]
    pub final_decision: Option<FinalDecision>,
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
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({"org_id": "o1"}),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
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

    #[tokio::test]
    async fn test_collecting_sink_in_runtime_context() {
        let sink = CollectingSink::new();
        sink.emit(AgentEvent::MessageDelta {
            text: "Hello".to_string(),
        })
        .await;
        sink.emit(AgentEvent::Done {
            final_message: Some("Hello".to_string()),
            usage: None,
        })
        .await;
        let events = sink.into_events();
        assert_eq!(events.len(), 2);
    }
}
