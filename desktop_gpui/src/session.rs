use contracts::chat::ChatEvent;
use web_sdk::{ChatTurnState, TurnStatus, reduce_chat_event};

/// UI-local generation guards async work before the shared wire reducer.
#[derive(Default)]
pub struct Conversation {
    pub generation: u64,
    pub session_id: Option<String>,
    pub messages: Vec<(String, String)>,
    pub turn: ChatTurnState,
    pub citations: std::collections::BTreeMap<usize, Vec<web_sdk::CitationView>>,
}

impl Conversation {
    pub fn begin(&mut self, query: String) -> Option<u64> {
        if query.trim().is_empty() || self.turn.status == TurnStatus::Streaming {
            return None;
        }
        if !self.turn.answer_text.is_empty() {
            self.citations.insert(self.messages.len(), self.turn.citations.iter().map(web_sdk::CitationView::from_value).collect());
            self.messages
                .push(("assistant".into(), self.turn.answer_text.clone()));
        }
        self.messages.push(("user".into(), query));
        self.generation += 1;
        self.turn = ChatTurnState {
            status: TurnStatus::Streaming,
            ..Default::default()
        };
        Some(self.generation)
    }

    pub fn event(&mut self, generation: u64, event: ChatEvent) {
        if generation != self.generation {
            return;
        }
        reduce_chat_event(&mut self.turn, event);
        if let Some(id) = &self.turn.session_id {
            self.session_id = Some(id.clone());
        }
    }

    pub fn cancel(&mut self) {
        if self.turn.status == TurnStatus::Streaming {
            self.turn.status = TurnStatus::Cancelled;
        }
    }

    pub fn reset(&mut self, session_id: Option<String>) -> u64 {
        self.generation += 1;
        self.session_id = session_id;
        self.messages.clear();
        self.citations.clear();
        self.turn = ChatTurnState::default();
        self.generation
    }

    pub fn restore(&mut self, messages: Vec<contracts::chat::ChatMessage>) {
        self.messages.clear();
        self.citations.clear();
        for message in messages.into_iter().filter(|m| m.role == "user" || m.role == "assistant") {
            let citations = message.citations.iter().filter_map(|c| serde_json::to_value(c).ok()).map(|v| web_sdk::CitationView::from_value(&v)).collect();
            self.citations.insert(self.messages.len(), citations);
            self.messages.push((message.role, message.content));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(json: serde_json::Value) -> ChatEvent {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn cancellation_preserves_partial_and_rejects_late_events() {
        let mut c = Conversation::default();
        let g = c.begin("你好".into()).unwrap();
        c.event(g, event(serde_json::json!({"event":"start","request_id":"r","session_id":"s","agent_type":"chat"})));
        c.event(g, event(serde_json::json!({"event":"token","request_id":"r","message_id":1,"content":"第一段"})));
        c.cancel();
        c.event(g, event(serde_json::json!({"event":"token","request_id":"r","message_id":1,"content":"迟到"})));
        assert_eq!(c.turn.answer_text, "第一段");
        assert_eq!(c.session_id.as_deref(), Some("s"));
        assert!(c.begin("下一轮".into()).is_some());
        assert_eq!(c.messages[1].1, "第一段");
    }

    #[test]
    fn old_generation_cannot_attach_session_to_new_chat() {
        let mut c = Conversation::default();
        let old = c.begin("问题".into()).unwrap();
        assert!(c.begin("重复发送".into()).is_none());
        c.reset(None);
        c.event(old, event(serde_json::json!({"event":"start","request_id":"old","session_id":"old","agent_type":"chat"})));
        assert!(c.session_id.is_none());
        assert!(c.messages.is_empty());
        assert!(c.begin("  ".into()).is_none());
    }
}
