use super::assembler::{AssembledContext, DisclosedState};
use crate::events::{AgentEvent, AgentEventSink};
use contracts::ToolCall;

/// Chunk size for non-streaming reasoning emission (matches stream token granularity).
const REASONING_CHUNK_CHARS: usize = 64;

/// Hard cap on the product-facing reasoning *summary* (not full CoT).
/// UI shows this under “思考摘要”; keep it short and scannable.
const REASONING_SUMMARY_MAX_CHARS: usize = 160;

fn truncate_reasoning_summary(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= REASONING_SUMMARY_MAX_CHARS {
        return text.to_string();
    }
    let hard: String = chars[..REASONING_SUMMARY_MAX_CHARS].iter().collect();
    // Prefer cutting at a soft boundary when possible.
    if let Some((byte_idx, ch)) = hard.char_indices().rev().find(|(_, c)| {
        matches!(
            c,
            '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';' | '\n' | '，' | ','
        )
    }) {
        if hard[..byte_idx].chars().count() >= REASONING_SUMMARY_MAX_CHARS / 2 {
            let mut out = hard[..byte_idx + ch.len_utf8()].to_string();
            out.push('…');
            return out;
        }
    }
    let mut out = hard;
    out.push('…');
    out
}

/// Emit `ReasoningSummaryDelta` events for non-empty reasoning text.
pub async fn emit_reasoning_chunks(sink: &dyn AgentEventSink, reasoning: Option<&str>) {
    let Some(raw) = reasoning.filter(|s| !s.is_empty()) else {
        return;
    };
    let text = truncate_reasoning_summary(raw);
    let chars: Vec<char> = text.chars().collect();
    for chunk in chars.chunks(REASONING_CHUNK_CHARS) {
        let piece: String = chunk.iter().collect();
        let _ = sink
            .emit(AgentEvent::ReasoningSummaryDelta { text: piece })
            .await;
    }
}

/// Record reasoning into an accumulator and emit SSE deltas.
pub async fn record_reasoning(
    sink: &dyn AgentEventSink,
    accumulator: &mut String,
    reasoning: Option<&str>,
) {
    emit_reasoning_chunks(sink, reasoning).await;
    if let Some(text) = reasoning.filter(|s| !s.is_empty()) {
        accumulator.push_str(text);
    }
}

/// Emit a full prompt snapshot for offline prompt-compliance analysis (requires `debug: true`).
pub async fn emit_prompt_snapshot(
    sink: &dyn AgentEventSink,
    phase: &str,
    iteration: u8,
    assembled: &AssembledContext,
    disclosed: &DisclosedState,
) {
    let disclosed_skills: Vec<String> = disclosed.disclosed_skill_ids.iter().cloned().collect();
    let payload = serde_json::json!({
        "phase": phase,
        "iteration": iteration,
        "disclosed_skills": disclosed_skills,
        "newly_disclosed_skills": assembled.newly_disclosed_skills,
        "system_content": assembled.system_content,
    });
    let _ = sink
        .emit(AgentEvent::DebugTrace {
            kind: "prompt_snapshot".to_string(),
            payload,
        })
        .await;
}

/// Emit structured plan telemetry (no LLM call) for offline trace_reasoning capture.
pub async fn emit_plan_decision_telemetry(
    sink: &dyn AgentEventSink,
    phase: &str,
    iteration: u8,
    assembled: &AssembledContext,
    disclosed: &DisclosedState,
) {
    let selected_skills: Vec<String> = disclosed.disclosed_skill_ids.iter().cloned().collect();
    let selected_tools: Vec<ToolCall> = assembled
        .tools
        .iter()
        .map(|tool| ToolCall {
            tool: tool.name.clone(),
            version: tool.version.clone(),
            args: serde_json::json!({}),
        })
        .collect();
    let reasoning = format!(
        "{phase} iteration {iteration}, skills: [{}]",
        selected_skills.join(", ")
    );
    let _ = sink
        .emit(AgentEvent::PlanDecision {
            selected_tools,
            selected_skills,
            selected_writing_styles: Vec::new(),
            behavior_mode: None,
            reasoning,
        })
        .await;
}

/// Emit iteration evaluation telemetry from loop state (no LLM eval call).
pub async fn emit_evaluation_telemetry(
    sink: &dyn AgentEventSink,
    iteration: u8,
    decision: &str,
    reasoning: &str,
    disclosed_skills: &[String],
    action_type: &str,
) {
    let signals = serde_json::json!({
        "iteration": iteration,
        "disclosed_skills": disclosed_skills,
        "action_type": action_type,
    });
    let _ = sink
        .emit(AgentEvent::Evaluation {
            signals: Some(signals),
            decision: decision.to_string(),
            reasoning: reasoning.to_string(),
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AgentEvent;
    use std::sync::{Arc, Mutex};

    struct RecordingSink {
        events: Arc<Mutex<Vec<AgentEvent>>>,
    }

    #[async_trait::async_trait]
    impl AgentEventSink for RecordingSink {
        async fn emit(&self, event: AgentEvent) -> Result<(), ()> {
            self.events.lock().unwrap().push(event);
            Ok(())
        }

        fn clone_boxed(&self) -> Box<dyn AgentEventSink> {
            Box::new(RecordingSink {
                events: self.events.clone(),
            })
        }
    }

    #[tokio::test]
    async fn emit_reasoning_chunks_skips_empty() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        emit_reasoning_chunks(&sink, None).await;
        emit_reasoning_chunks(&sink, Some("")).await;
        assert!(events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn emit_reasoning_chunks_splits_and_caps_summary() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        let text = "x".repeat(300);
        emit_reasoning_chunks(&sink, Some(&text)).await;
        let evs = events.lock().unwrap();
        let joined: String = evs
            .iter()
            .map(|ev| match ev {
                AgentEvent::ReasoningSummaryDelta { text } => text.as_str(),
                _ => "",
            })
            .collect();
        // Cap at REASONING_SUMMARY_MAX_CHARS + ellipsis.
        assert!(joined.chars().count() <= REASONING_SUMMARY_MAX_CHARS + 1);
        assert!(joined.ends_with('…'));
        assert!(evs.len() >= 2);
        assert!(matches!(
            &evs[0],
            AgentEvent::ReasoningSummaryDelta { text } if text.chars().count() == REASONING_CHUNK_CHARS
        ));
    }

    #[tokio::test]
    async fn record_reasoning_accumulates_raw_but_emits_capped() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        let mut acc = String::new();
        record_reasoning(&sink, &mut acc, Some("alpha")).await;
        record_reasoning(&sink, &mut acc, Some("beta")).await;
        assert_eq!(acc, "alphabeta");
        assert_eq!(events.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn emit_prompt_snapshot_includes_system_content() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        let mut disclosed = DisclosedState::default();
        disclosed
            .disclosed_skill_ids
            .insert("rag-answer".to_string());
        let assembled = AssembledContext {
            system_content: "You are RAG.".to_string(),
            tools: Vec::new(),
            newly_disclosed_skills: vec!["rag-answer".to_string()],
        };
        emit_prompt_snapshot(&sink, "retrieve", 0, &assembled, &disclosed).await;
        let ev = events.lock().unwrap().pop().unwrap();
        match ev {
            AgentEvent::DebugTrace { kind, payload } => {
                assert_eq!(kind, "prompt_snapshot");
                assert_eq!(payload["phase"], "retrieve");
                assert_eq!(payload["system_content"], "You are RAG.");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn emit_plan_decision_telemetry_includes_skills() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        let mut disclosed = DisclosedState::default();
        disclosed.disclosed_skill_ids.insert("rag-plan".to_string());
        let assembled = AssembledContext {
            system_content: "sys".to_string(),
            tools: vec![contracts::ToolSpec {
                name: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                description: String::new(),
                input_schema: serde_json::json!({}),
                output_schema: serde_json::json!({}),
            }],
            newly_disclosed_skills: vec![],
        };
        emit_plan_decision_telemetry(&sink, "retrieve", 1, &assembled, &disclosed).await;
        let ev = events.lock().unwrap().pop().unwrap();
        match ev {
            AgentEvent::PlanDecision {
                selected_skills,
                reasoning,
                ..
            } => {
                assert_eq!(selected_skills, vec!["rag-plan".to_string()]);
                assert!(reasoning.contains("retrieve iteration 1"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn emit_evaluation_telemetry_includes_decision_and_reasoning() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = RecordingSink {
            events: events.clone(),
        };
        emit_evaluation_telemetry(
            &sink,
            2,
            "native_tool_call",
            "2 tool calls",
            &["rag-plan".to_string()],
            "native_tool_call",
        )
        .await;
        let ev = events.lock().unwrap().pop().unwrap();
        match ev {
            AgentEvent::Evaluation {
                decision,
                reasoning,
                signals,
            } => {
                assert_eq!(decision, "native_tool_call");
                assert_eq!(reasoning, "2 tool calls");
                assert_eq!(signals.unwrap()["iteration"], 2);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
