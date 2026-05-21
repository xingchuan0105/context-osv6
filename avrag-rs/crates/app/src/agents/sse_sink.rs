//! SSE sink: bridges `AgentEvent` into the existing `ChatEvent` SSE transport.
//!
//! `SseSink` receives agent-level events and maps them to the frontend-facing `ChatEvent`
//! variants so that the handler can reuse `sse_response_from_receiver` without change.

use crate::agents::events::{AgentEvent, AgentEventSink};
use contracts::chat::ChatEvent;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::UnboundedSender;

/// Sink that maps `AgentEvent` to `ChatEvent` and forwards them into the
/// SSE channel used by the HTTP handlers.
pub struct SseSink {
    sender: UnboundedSender<ChatEvent>,
    request_id: String,
    session_id: String,
    message_id: i64,
    agent_type: String,
    emit_done: bool,
    emit_debug_trace: bool,
    answer_started: AtomicBool,
    message_delta_emitted: AtomicBool,
}

impl Clone for SseSink {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            request_id: self.request_id.clone(),
            session_id: self.session_id.clone(),
            message_id: self.message_id,
            agent_type: self.agent_type.clone(),
            emit_done: self.emit_done,
            emit_debug_trace: self.emit_debug_trace,
            answer_started: AtomicBool::new(self.answer_started.load(Ordering::SeqCst)),
            message_delta_emitted: AtomicBool::new(self.message_delta_emitted.load(Ordering::SeqCst)),
        }
    }
}

impl SseSink {
    pub fn new(
        sender: UnboundedSender<ChatEvent>,
        request_id: String,
        session_id: String,
        message_id: i64,
    ) -> Self {
        Self::new_with_agent_type(
            sender,
            request_id,
            session_id,
            message_id,
            "chat".to_string(),
        )
    }

    pub fn new_with_agent_type(
        sender: UnboundedSender<ChatEvent>,
        request_id: String,
        session_id: String,
        message_id: i64,
        agent_type: String,
    ) -> Self {
        Self {
            sender,
            request_id,
            session_id,
            message_id,
            agent_type,
            emit_done: true,
            emit_debug_trace: false,
            answer_started: AtomicBool::new(false),
            message_delta_emitted: AtomicBool::new(false),
        }
    }

    pub fn without_done_event(mut self) -> Self {
        self.emit_done = false;
        self
    }

    pub fn with_debug_trace(mut self, enabled: bool) -> Self {
        self.emit_debug_trace = enabled;
        self
    }

    pub fn has_message_delta(&self) -> bool {
        self.message_delta_emitted.load(Ordering::SeqCst)
    }

    /// Send a single `AgentEvent` after mapping it to `ChatEvent`.
    pub fn send(&self, event: AgentEvent) {
        if matches!(event, AgentEvent::Done { .. }) && !self.emit_done {
            return;
        }
        if matches!(event, AgentEvent::DebugTrace { .. }) && !self.emit_debug_trace {
            return;
        }
        // Audit records are internal — never forwarded to the client SSE stream.
        if matches!(event, AgentEvent::Audit { .. }) {
            return;
        }
        let chat_event = self.map_event(event);
        let _ = self.sender.send(chat_event);
    }

    fn map_event(&self, event: AgentEvent) -> ChatEvent {
        match event {
            AgentEvent::Activity { stage, message } => ChatEvent::Activity {
                request_id: self.request_id.clone(),
                phase: stage,
                title: message,
                detail: None,
                counts: BTreeMap::new(),
                sources_preview: Vec::new(),
                timestamp: Some(now_rfc3339()),
            },
            AgentEvent::ReasoningSummaryDelta { text } => ChatEvent::ReasoningSummaryDelta {
                request_id: self.request_id.clone(),
                message_id: self.message_id,
                content: text,
            },
            AgentEvent::MessageDelta { text } => ChatEvent::Token {
                request_id: self.request_id.clone(),
                message_id: self.message_id,
                content: text,
            },
            AgentEvent::Citations { citations } => ChatEvent::Citations {
                request_id: self.request_id.clone(),
                message_id: self.message_id,
                citations: citations
                    .into_iter()
                    .filter_map(|c| serde_json::to_value(c).ok())
                    .collect(),
            },
            AgentEvent::Usage {
                provider,
                model,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                request_count,
                metadata,
            } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: "usage".to_string(),
                status: "ok".to_string(),
                detail: Some(serde_json::json!({
                    "provider": provider,
                    "model": model,
                    "prompt_tokens": prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "total_tokens": total_tokens,
                    "request_count": request_count,
                    "metadata": metadata,
                })),
            },
            AgentEvent::Done {
                final_message,
                usage,
            } => {
                let answer = final_message.clone().unwrap_or_default();
                ChatEvent::Done {
                    request_id: self.request_id.clone(),
                    session_id: self.session_id.clone(),
                    message_id: self.message_id,
                    payload: serde_json::json!({
                        "session_id": self.session_id,
                        "message_id": self.message_id,
                        "agent_type": self.agent_type,
                        "answer": answer,
                        "final_message": final_message,
                        "usage": usage,
                    }),
                }
            }
            AgentEvent::Error { code, message } => ChatEvent::Error {
                request_id: self.request_id.clone(),
                code,
                message,
            },
            AgentEvent::DebugTrace { kind, payload } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: kind,
                status: "debug".to_string(),
                detail: Some(payload),
            },
            AgentEvent::StateTransition {
                transition_type,
                state_id,
                state_kind,
                elapsed_ms,
                payload,
            } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: format!("state_{}", serde_json::to_string(&transition_type).unwrap_or_default().trim_matches('"')),
                status: "ok".to_string(),
                detail: Some(serde_json::json!({
                    "state_id": state_id,
                    "state_kind": state_kind,
                    "elapsed_ms": elapsed_ms,
                    "payload": payload,
                })),
            },
            AgentEvent::PlanDecision {
                selected_tools,
                selected_skills,
                reasoning,
            } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: "plan_decision".to_string(),
                status: "ok".to_string(),
                detail: Some(serde_json::json!({
                    "selected_tools": selected_tools,
                    "selected_skills": selected_skills,
                    "reasoning": reasoning,
                })),
            },
            AgentEvent::ToolResult {
                tool,
                status,
                data,
                elapsed_ms,
            } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: format!("tool_result.{}", tool),
                status: format!("{:?}", status).to_lowercase(),
                detail: Some(serde_json::json!({
                    "tool": tool,
                    "status": status,
                    "data": data,
                    "elapsed_ms": elapsed_ms,
                })),
            },
            AgentEvent::Evaluation {
                signals,
                decision,
                reasoning,
            } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: "evaluation".to_string(),
                status: "ok".to_string(),
                detail: Some(serde_json::json!({
                    "signals": signals,
                    "decision": decision,
                    "reasoning": reasoning,
                })),
            },
            AgentEvent::BudgetTick { current, max } => ChatEvent::Trace {
                request_id: self.request_id.clone(),
                stage: "budget_tick".to_string(),
                status: "ok".to_string(),
                detail: Some(serde_json::json!({
                    "current": current,
                    "max": max,
                })),
            },
            AgentEvent::Audit { .. } => {
                unreachable!("Audit events are filtered in on_event before map_event")
            }
        }
    }

    fn ensure_answer_started(&self) -> Result<(), ()> {
        if self
            .answer_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.sender.send(ChatEvent::AnswerStart {
                request_id: self.request_id.clone(),
                session_id: self.session_id.clone(),
                message_id: self.message_id,
                agent_type: self.agent_type.clone(),
            }).map_err(|_| ())?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AgentEventSink for SseSink {
    async fn emit(&self, event: AgentEvent) -> Result<(), ()> {
        if matches!(event, AgentEvent::MessageDelta { .. }) {
            self.ensure_answer_started()?;
            self.message_delta_emitted.store(true, Ordering::SeqCst);
        }
        self.send(event);
        Ok(())
    }

    fn clone_boxed(&self) -> Box<dyn AgentEventSink> {
        Box::new(self.clone())
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::{AgentEvent, AgentEventSink};
    use common::Citation;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn test_sse_sink_maps_activity() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 42);
        sink.send(AgentEvent::Activity {
            stage: "planning".to_string(),
            message: " analysing".to_string(),
        });
        let event = rx.try_recv().expect("activity event should be sent");
        assert!(
            matches!(event, ChatEvent::Activity { request_id, phase, title, .. }
                if request_id == "req-1" && phase == "planning" && title == " analysing"
            )
        );
    }

    #[test]
    fn test_sse_sink_maps_message_delta() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 7);
        sink.send(AgentEvent::MessageDelta {
            text: "hello".to_string(),
        });
        let event = rx.try_recv().expect("token event should be sent");
        assert!(
            matches!(event, ChatEvent::Token { request_id, message_id, content }
                if request_id == "req-1" && message_id == 7 && content == "hello"
            )
        );
    }

    #[tokio::test]
    async fn test_sse_sink_is_agent_event_sink_and_starts_answer_before_first_delta() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 7);
        assert!(!sink.has_message_delta());

        sink.emit(AgentEvent::MessageDelta {
            text: "hello".to_string(),
        })
        .await;
        assert!(sink.has_message_delta());

        let first = rx.try_recv().expect("answer_start should be sent before first delta");
        assert!(
            matches!(first, ChatEvent::AnswerStart { request_id, session_id, message_id, agent_type }
                if request_id == "req-1" && session_id == "sess-1" && message_id == 7 && agent_type == "chat"
            )
        );

        let second = rx.try_recv().expect("token event should follow answer_start");
        assert!(
            matches!(second, ChatEvent::Token { request_id, message_id, content }
                if request_id == "req-1" && message_id == 7 && content == "hello"
            )
        );
    }

    #[test]
    fn test_sse_sink_maps_citations() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 1);
        sink.send(AgentEvent::Citations {
            citations: vec![Citation {
                citation_id: 1,
                doc_id: "d1".to_string(),
                chunk_id: None,
                page: None,
                doc_name: "doc".to_string(),
                preview: None,
                content: None,
                score: 1.0,
                layer: None,
                chunk_type: None,
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }],
        });
        let event = rx.try_recv().expect("citations event should be sent");
        assert!(
            matches!(event, ChatEvent::Citations { request_id, message_id, .. }
                if request_id == "req-1" && message_id == 1
            )
        );
    }

    #[test]
    fn test_sse_sink_maps_error() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 0);
        sink.send(AgentEvent::Error {
            code: "E404".to_string(),
            message: "not found".to_string(),
        });
        let event = rx.try_recv().expect("error event should be sent");
        assert!(
            matches!(event, ChatEvent::Error { request_id, code, message }
                if request_id == "req-1" && code == "E404" && message == "not found"
            )
        );
    }

    #[test]
    fn test_sse_sink_maps_done() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 99);
        sink.send(AgentEvent::Done {
            final_message: Some("done".to_string()),
            usage: None,
        });
        let event = rx.try_recv().expect("done event should be sent");
        assert!(
            matches!(&event, ChatEvent::Done { request_id, session_id, message_id, .. }
                if request_id == "req-1" && session_id == "sess-1" && *message_id == 99
            )
        );
        let ChatEvent::Done { payload, .. } = event else {
            unreachable!();
        };
        assert_eq!(
            payload.get("agent_type").and_then(|value| value.as_str()),
            Some("chat")
        );
        assert_eq!(
            payload.get("answer").and_then(|value| value.as_str()),
            Some("done")
        );
        assert_eq!(
            payload
                .get("final_message")
                .and_then(|value| value.as_str()),
            Some("done")
        );
    }

    #[test]
    fn test_sse_sink_suppresses_debug_trace_by_default() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink = SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 0);
        sink.send(AgentEvent::DebugTrace {
            kind: "search.execution".to_string(),
            payload: serde_json::json!({"internal": true}),
        });
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_sse_sink_maps_debug_trace_when_enabled() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink =
            SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 0).with_debug_trace(true);
        sink.send(AgentEvent::DebugTrace {
            kind: "search.execution".to_string(),
            payload: serde_json::json!({"internal": true}),
        });
        let event = rx.try_recv().expect("debug trace event should be sent when enabled");
        assert!(matches!(event, ChatEvent::Trace { stage, status, .. }
            if stage == "search.execution" && status == "debug"));
    }

    #[test]
    fn test_sse_sink_can_suppress_done() {
        let (tx, mut rx) = unbounded_channel::<ChatEvent>();
        let sink =
            SseSink::new(tx, "req-1".to_string(), "sess-1".to_string(), 99).without_done_event();
        sink.send(AgentEvent::Done {
            final_message: Some("done".to_string()),
            usage: None,
        });
        assert!(rx.try_recv().is_err());
    }
}
