use avrag_llm::ChatMessage;

use super::assembler::LoopPhase;
use super::config::ModeConfig;
use crate::runtime::AgentRequest;

pub struct LoopContext<'a> {
    pub mode: &'a ModeConfig,
    pub request: &'a AgentRequest,
    pub iteration: u8,
    pub phase: LoopPhase,
    pub has_retrieval_observation: bool,
    pub base_message_count: usize,
}

pub trait LoopHooks: Send + Sync {
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        let _ = (messages, ctx);
    }

    fn convert_to_llm(&self, messages: &[ChatMessage]) -> Vec<ChatMessage> {
        messages.to_vec()
    }
}

pub struct StandardLoopHooks {
    pub max_react_messages: usize,
}

impl Default for StandardLoopHooks {
    fn default() -> Self {
        Self {
            max_react_messages: 20,
        }
    }
}

impl LoopHooks for StandardLoopHooks {
    /// Truncate the conversation to keep it within `max_react_messages` of the
    /// protected prefix, **without ever splitting an `assistant(tool_calls)` /
    /// `tool` result pair**.
    ///
    /// OpenAI-format requires every `assistant` message carrying `tool_calls`
    /// to be *immediately* followed by the matching `tool` messages (keyed by
    /// `tool_call_id`). The tool results always come *after* the assistant
    /// tool-calls that produced them, so the only way a blind middle-range
    /// drain can corrupt a pair is by deleting one half — leaving either an
    /// orphan `tool` message whose parent was removed, or a dangling
    /// `assistant(tool_calls)` whose results were removed. Either produces a
    /// provider 400.
    ///
    /// To avoid this we never cut *inside* a turn. We compute the drainable
    /// region `[base_message_count .. suffix_start)` (everything between the
    /// protected prefix and the protected suffix) and then *realign the drain
    /// end forward* past any leading non-`assistant` messages of the would-be
    /// protected suffix. If the kept region would otherwise begin on a `tool`
    /// message (whose `assistant(tool_calls)` parent is being drained), we
    /// advance the cut so the kept region starts on an `assistant` turn
    /// boundary — keeping every `tool` result attached to its parent. This
    /// shrinks the protected suffix by at most one turn, always preferable to
    /// a provider 400.
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        let base = ctx.base_message_count;
        if messages.len() <= base + self.max_react_messages {
            return;
        }
        // Start of the protected suffix (the most recent max_react_messages).
        let suffix_start = messages.len() - self.max_react_messages;
        if suffix_start <= base {
            return; // protected region already covers everything
        }
        // Realign the drain end FORWARD past any leading non-`assistant`
        // messages of the would-be protected suffix. A `tool` (or other
        // non-assistant) message at the head of the kept region would be an
        // orphan: its `assistant(tool_calls)` parent lives further left and is
        // about to be drained. Advancing `suffix_start` so the kept region
        // begins on an `assistant` turn boundary keeps every tool message
        // attached to its parent. This shrinks the protected suffix slightly
        // (at most one turn) — always preferable to a provider 400.
        let mut drain_end = suffix_start;
        while drain_end < messages.len() && !is_assistant_turn_boundary(&messages[drain_end]) {
            drain_end += 1;
        }
        if drain_end > base {
            messages.drain(base..drain_end);
        }
    }
}

/// An `assistant` message marks the *start* of a turn; everything after it up
/// to the next assistant message (the `tool` results and any follow-up) belongs
/// to that turn. Cutting the drain range so the *first kept* message is an
/// assistant boundary therefore guarantees no `tool` message is ever left
/// without its preceding `assistant(tool_calls)` parent.
fn is_assistant_turn_boundary(msg: &ChatMessage) -> bool {
    msg.role == "assistant"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_loop::config;
    use crate::runtime::AgentRequest;

    /// Build an assistant message carrying OpenAI-format `tool_calls`.
    fn assistant_with_tool_calls(call_id: &str) -> ChatMessage {
        ChatMessage {
            role: "assistant".to_string(),
            content: "thinking".to_string(),
            multimodal_content: None,
            name: None,
            tool_call_id: None,
            tool_calls: Some(serde_json::json!([{
                "id": call_id,
                "type": "function",
                "function": { "name": "dense_retrieval", "arguments": "{}" }
            }])),
            reasoning_content: None,
        }
    }

    /// Build a `tool` result message keyed by `tool_call_id`.
    fn tool_result(call_id: &str, payload: &str) -> ChatMessage {
        ChatMessage {
            role: "tool".to_string(),
            content: payload.to_string(),
            multimodal_content: None,
            name: None,
            tool_call_id: Some(call_id.to_string()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    fn ctx(messages_len: usize, base: usize) -> LoopContext<'static> {
        // SAFETY of borrow: `transform_context` only reads `base_message_count`,
        // so the `mode`/`request` references are never dereferenced; we hand
        // dangling-but-unused references to satisfy the struct shape. To keep
        // this sound without `unsafe`, we instead build real (cheap) values and
        // leak them so the `'static` lifetime holds for the test.
        static MODE: std::sync::OnceLock<ModeConfig> = std::sync::OnceLock::new();
        static REQUEST: std::sync::OnceLock<AgentRequest> = std::sync::OnceLock::new();
        let mode = MODE.get_or_init(|| config::load_mode_config("rag").unwrap());
        let request = REQUEST.get_or_init(|| AgentRequest {
            kind: crate::AgentKind::Rag,
            query: "test".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth: crate::runtime::stub_agent_auth(),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
        });
        let _ = messages_len;
        LoopContext {
            mode,
            request,
            iteration: 0,
            phase: LoopPhase::Retrieve,
            has_retrieval_observation: false,
            base_message_count: base,
        }
    }

    /// Collect the set of `tool_call_id`s declared by assistant `tool_calls`.
    fn declared_tool_call_ids(messages: &[ChatMessage]) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for m in messages {
            if m.role == "assistant" {
                if let Some(tc) = m.tool_calls.as_ref().and_then(|v| v.as_array()) {
                    for entry in tc {
                        if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                            set.insert(id.to_string());
                        }
                    }
                }
            }
        }
        set
    }

    #[test]
    fn preserves_tool_call_pairing_under_truncation() {
        // [base: 2 prefix msgs][4 full turns (assistant+tool)][suffix tail]
        // max_react_messages is small so the middle turns get drained.
        let hooks = StandardLoopHooks { max_react_messages: 3 };

        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
        ];
        // Four complete turns in the drainable middle.
        for i in 0..4 {
            let id = format!("call_{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, &format!("result-{i}")));
        }
        // Protected suffix: a final assistant turn that must survive intact.
        messages.push(assistant_with_tool_calls("call_keep"));
        messages.push(tool_result("call_keep", "keep-result"));
        messages.push(ChatMessage::user("thanks"));

        let before_len = messages.len();
        let base = 2;
        hooks.transform_context(&mut messages, &ctx(before_len, base));

        // (a) total length reduced
        assert!(messages.len() < before_len, "expected truncation");
        // (b) protected prefix untouched
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
        assert_eq!(messages[..base].len(), base);
        // (c) no orphan tool message without a preceding matching assistant
        let declared = declared_tool_call_ids(&messages);
        for m in &messages {
            if m.role == "tool" {
                let id = m.tool_call_id.as_ref().expect("tool msg has tool_call_id");
                assert!(
                    declared.contains(id),
                    "orphan tool message for id {id} survived (no matching assistant tool_calls)"
                );
            }
        }
        // The protected suffix turn survived intact.
        assert!(declared.contains("call_keep"));
        assert!(messages.iter().any(|m| m.role == "tool"
            && m.tool_call_id.as_deref() == Some("call_keep")));
    }

    #[test]
    fn does_not_truncate_when_under_budget() {
        let hooks = StandardLoopHooks { max_react_messages: 20 };
        let mut messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            assistant_with_tool_calls("c1"),
            tool_result("c1", "r1"),
        ];
        let before_len = messages.len();
        hooks.transform_context(&mut messages, &ctx(before_len, 2));
        assert_eq!(messages.len(), before_len, "nothing should be drained under budget");
        // Prefix untouched.
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[2].role, "assistant");
    }

    #[test]
    fn truncation_realigns_to_assistant_boundary() {
        // Force the suffix to begin exactly on a `tool` message, so a naive
        // drain would orphan it. The pairing-aware logic must pull the drain end
        // back to include its parent assistant(tool_calls).
        let hooks = StandardLoopHooks { max_react_messages: 2 };

        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            // Turn A (should be dropped entirely, pair stays together).
            assistant_with_tool_calls("a"),
            tool_result("a", "ra"),
            // Turn B: assistant + tool — these two form the protected suffix,
            // starting on the `tool` message relative to the naive cut.
            assistant_with_tool_calls("b"),
            tool_result("b", "rb"),
        ];
        let before_len = messages.len();
        hooks.transform_context(&mut messages, &ctx(before_len, 2));

        let declared = declared_tool_call_ids(&messages);
        for m in &messages {
            if m.role == "tool" {
                let id = m.tool_call_id.as_ref().unwrap();
                assert!(declared.contains(id), "orphan tool {id} survived");
            }
        }
        // Prefix preserved.
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
    }
}
