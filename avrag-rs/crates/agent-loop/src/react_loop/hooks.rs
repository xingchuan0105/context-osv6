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

/// Result of [`LoopHooks::before_tool_call`].
///
/// **Default is never block.** Product allow/deny/tier stays in
/// `PolicyEnforcer` inside `dispatch_tool`. A hook that sets `block: true`
/// is for tests / host-level emergency intercepts only — do not recreate
/// policy tables here (plan Wave B2 / D7).
#[derive(Debug, Clone, Default)]
pub struct BeforeToolCallOutcome {
    pub block: bool,
    pub reason: Option<String>,
}

/// Per-turn context transforms and observation points for the retrieve loop.
///
/// **Policy boundary (Wave A–B / plan 2026-07-29):** tool allow/deny/tier lives in
/// `agent_tools::PolicyEnforcer` + `ToolCatalog` metadata — **not** here.
/// Hooks may observe, record, or *delegate* to the enforcer; they must not grow
/// a parallel allowlist/denylist.
///
/// Prefer implementing this trait and calling [`crate::react_loop::ReActLoop::run_with_hooks`]
/// over forking `ReActLoop::run`.
pub trait LoopHooks: Send + Sync {
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        let _ = (messages, ctx);
    }

    /// Map messages at the LLM API boundary (after system assemble).
    fn convert_to_llm(&self, messages: &[ChatMessage]) -> Vec<ChatMessage> {
        messages.to_vec()
    }

    /// Observability / rare host intercept. Default: allow (never block).
    fn before_tool_call(&self, tool: &str, args: &serde_json::Value) -> BeforeToolCallOutcome {
        let _ = (tool, args);
        BeforeToolCallOutcome::default()
    }

    /// Observability after a native tool finishes (status is snake-ish debug label).
    fn after_tool_call(&self, tool: &str, status: &str) {
        let _ = (tool, status);
    }

    /// Observability at end of a retrieve iteration (`continue` / `break` / `direct`).
    fn on_turn_end(&self, iteration: u8, control: &str) {
        let _ = (iteration, control);
    }
}

/// Default retrieve-loop message windowing.
///
/// Two-tier compaction (prefix-cache friendly, plan 2026-07-04 P3-2 / Wave A4):
/// - While `len <= base + compact_high_watermark`: **append-only** (no drain).
/// - When `len > base + compact_high_watermark`: one-shot drain of the middle so
///   the protected suffix is about `max_react_messages` long (pair-safe).
///
/// Set `compact_high_watermark == max_react_messages` to recover the pre-A4
/// “drain whenever over low watermark” cadence (used by characterization tests).
pub struct StandardLoopHooks {
    /// Protected suffix size after compaction (low watermark). Default 20.
    pub max_react_messages: usize,
    /// Append-only ceiling above `base` before compaction fires. Default 32.
    pub compact_high_watermark: usize,
}

impl Default for StandardLoopHooks {
    fn default() -> Self {
        Self {
            max_react_messages: 20,
            compact_high_watermark: 32,
        }
    }
}

impl LoopHooks for StandardLoopHooks {
    /// Truncate the conversation when over the high watermark, keeping
    /// `max_react_messages` of the protected suffix **without ever splitting an
    /// `assistant(tool_calls)` / `tool` result pair**.
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
    /// region `[base_message_count .. suffix_start)` and then *realign the drain
    /// end forward* past any leading non-`assistant` messages of the would-be
    /// protected suffix.
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        let base = ctx.base_message_count;
        let high = self.compact_high_watermark.max(self.max_react_messages);
        // Append-only until the high watermark is exceeded.
        if messages.len() <= base + high {
            return;
        }
        // Target: keep the most recent `max_react_messages` after the prefix.
        let suffix_start = messages.len() - self.max_react_messages;
        if suffix_start <= base {
            return; // protected region already covers everything
        }
        // Realign the drain end FORWARD past any leading non-`assistant`
        // messages of the would-be protected suffix.
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

    /// Legacy single-tier cadence: high == low (pre-A4 drain threshold).
    fn legacy_hooks(max_react_messages: usize) -> StandardLoopHooks {
        StandardLoopHooks {
            max_react_messages,
            compact_high_watermark: max_react_messages,
        }
    }

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

    fn ctx(base: usize) -> LoopContext<'static> {
        static MODE: std::sync::OnceLock<ModeConfig> = std::sync::OnceLock::new();
        static REQUEST: std::sync::OnceLock<AgentRequest> = std::sync::OnceLock::new();
        let mode = MODE.get_or_init(|| config::load_mode_config("rag").unwrap());
        let request = REQUEST.get_or_init(|| AgentRequest {
            kind: crate::AgentKind::Rag,
            query: "test".to_string(),
            workspace_id: None,
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
        LoopContext {
            mode,
            request,
            iteration: 0,
            phase: LoopPhase::Retrieve,
            has_retrieval_observation: false,
            base_message_count: base,
        }
    }

    fn role_sequence(messages: &[ChatMessage]) -> Vec<&str> {
        messages.iter().map(|m| m.role.as_str()).collect()
    }

    fn tool_id_sequence(messages: &[ChatMessage]) -> Vec<Option<&str>> {
        messages
            .iter()
            .map(|m| {
                if m.role == "tool" {
                    m.tool_call_id.as_deref()
                } else if m.role == "assistant" {
                    m.tool_calls
                        .as_ref()
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|e| e.get("id"))
                        .and_then(|i| i.as_str())
                } else {
                    None
                }
            })
            .collect()
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

    fn assert_no_orphan_tools(messages: &[ChatMessage]) {
        let declared = declared_tool_call_ids(messages);
        for m in messages {
            if m.role == "tool" {
                let id = m.tool_call_id.as_ref().expect("tool msg has tool_call_id");
                assert!(
                    declared.contains(id),
                    "orphan tool message for id {id} survived (no matching assistant tool_calls)"
                );
            }
        }
    }

    // ── A0 characterization: full role / tool-id sequence snapshots ─────────

    #[test]
    fn characterization_role_sequence_legacy_tier_drain() {
        // base=2, low=high=3 → drain as soon as len > 5.
        let hooks = legacy_hooks(3);
        let mut messages: Vec<ChatMessage> =
            vec![ChatMessage::system("sys"), ChatMessage::user("q")];
        for i in 0..4 {
            let id = format!("call_{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, &format!("result-{i}")));
        }
        messages.push(assistant_with_tool_calls("call_keep"));
        messages.push(tool_result("call_keep", "keep-result"));
        messages.push(ChatMessage::user("thanks"));

        // len = 2 + 8 + 3 = 13; high = 3 → drain middle to keep last 3.
        assert_eq!(messages.len(), 13);
        hooks.transform_context(&mut messages, &ctx(2));

        assert_eq!(
            role_sequence(&messages),
            vec!["system", "user", "assistant", "tool", "user"]
        );
        assert_eq!(
            tool_id_sequence(&messages),
            vec![None, None, Some("call_keep"), Some("call_keep"), None]
        );
        assert_no_orphan_tools(&messages);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
    }

    #[test]
    fn characterization_no_drain_under_high_watermark() {
        // Default production hooks: high=32, low=20. A short trace stays intact.
        let hooks = StandardLoopHooks::default();
        let mut messages: Vec<ChatMessage> =
            vec![ChatMessage::system("sys"), ChatMessage::user("q")];
        for i in 0..5 {
            let id = format!("c{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, "r"));
        }
        let before: Vec<String> = role_sequence(&messages)
            .into_iter()
            .map(str::to_string)
            .collect();
        let before_len = messages.len();
        hooks.transform_context(&mut messages, &ctx(2));
        assert_eq!(
            messages.len(),
            before_len,
            "append-only under high watermark"
        );
        assert_eq!(
            role_sequence(&messages)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>(),
            before
        );
    }

    // ── Pair-safety (legacy threshold = high == low) ────────────────────────

    #[test]
    fn preserves_tool_call_pairing_under_truncation() {
        let hooks = legacy_hooks(3);

        let mut messages: Vec<ChatMessage> =
            vec![ChatMessage::system("sys"), ChatMessage::user("q")];
        for i in 0..4 {
            let id = format!("call_{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, &format!("result-{i}")));
        }
        messages.push(assistant_with_tool_calls("call_keep"));
        messages.push(tool_result("call_keep", "keep-result"));
        messages.push(ChatMessage::user("thanks"));

        let before_len = messages.len();
        let base = 2;
        hooks.transform_context(&mut messages, &ctx(base));

        assert!(messages.len() < before_len, "expected truncation");
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
        assert_no_orphan_tools(&messages);
        let declared = declared_tool_call_ids(&messages);
        assert!(declared.contains("call_keep"));
        assert!(
            messages
                .iter()
                .any(|m| m.role == "tool" && m.tool_call_id.as_deref() == Some("call_keep"))
        );
    }

    #[test]
    fn does_not_truncate_when_under_budget() {
        let hooks = legacy_hooks(20);
        let mut messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            assistant_with_tool_calls("c1"),
            tool_result("c1", "r1"),
        ];
        let before_len = messages.len();
        hooks.transform_context(&mut messages, &ctx(2));
        assert_eq!(
            messages.len(),
            before_len,
            "nothing should be drained under budget"
        );
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[2].role, "assistant");
    }

    #[test]
    fn truncation_realigns_to_assistant_boundary() {
        let hooks = legacy_hooks(2);

        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("q"),
            assistant_with_tool_calls("a"),
            tool_result("a", "ra"),
            assistant_with_tool_calls("b"),
            tool_result("b", "rb"),
        ];
        let before_len = messages.len();
        hooks.transform_context(&mut messages, &ctx(2));

        assert!(messages.len() <= before_len);
        assert_no_orphan_tools(&messages);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
    }

    // ── A4 two-tier compaction ──────────────────────────────────────────────

    #[test]
    fn two_tier_appends_until_high_watermark() {
        let hooks = StandardLoopHooks {
            max_react_messages: 4,
            compact_high_watermark: 8,
        };
        let mut messages: Vec<ChatMessage> =
            vec![ChatMessage::system("sys"), ChatMessage::user("q")];
        // 3 full turns = 6 msgs → total 8 = base+6 ≤ base+8 → no drain.
        for i in 0..3 {
            let id = format!("t{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, "r"));
        }
        assert_eq!(messages.len(), 8);
        hooks.transform_context(&mut messages, &ctx(2));
        assert_eq!(messages.len(), 8, "still under high watermark");
    }

    #[test]
    fn two_tier_compacts_once_over_high_watermark() {
        let hooks = StandardLoopHooks {
            max_react_messages: 4,
            compact_high_watermark: 8,
        };
        let mut messages: Vec<ChatMessage> =
            vec![ChatMessage::system("sys"), ChatMessage::user("q")];
        // 5 turns = 10 → total 12 > base+8 → compact, keep last 4 (+ pair realign).
        for i in 0..5 {
            let id = format!("t{i}");
            messages.push(assistant_with_tool_calls(&id));
            messages.push(tool_result(&id, "r"));
        }
        assert_eq!(messages.len(), 12);
        hooks.transform_context(&mut messages, &ctx(2));
        assert!(messages.len() < 12, "must compact over high watermark");
        assert!(messages.len() >= 2 + 4 || messages.len() > 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "q");
        assert_no_orphan_tools(&messages);
        // Latest turns should survive.
        let declared = declared_tool_call_ids(&messages);
        assert!(declared.contains("t4"));
    }

    #[test]
    fn custom_hook_is_invokable_via_trait_object() {
        struct CountingHook {
            hits: std::sync::atomic::AtomicUsize,
        }
        impl LoopHooks for CountingHook {
            fn transform_context(&self, messages: &mut Vec<ChatMessage>, _ctx: &LoopContext) {
                self.hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // No mutation — proves injection without changing sequences.
                let _ = messages;
            }
        }
        let hook = CountingHook {
            hits: std::sync::atomic::AtomicUsize::new(0),
        };
        let hooks: &dyn LoopHooks = &hook;
        let mut messages = vec![ChatMessage::user("x")];
        hooks.transform_context(&mut messages, &ctx(0));
        assert_eq!(hook.hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn before_tool_call_default_never_blocks() {
        let hooks = StandardLoopHooks::default();
        let outcome = hooks.before_tool_call("web_search", &serde_json::json!({"q": "x"}));
        assert!(!outcome.block);
        assert!(outcome.reason.is_none());
    }

    #[test]
    fn convert_to_llm_default_is_identity() {
        let hooks = StandardLoopHooks::default();
        let msgs = vec![ChatMessage::user("hello"), ChatMessage::assistant("hi")];
        let out = hooks.convert_to_llm(&msgs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].content, "hello");
        assert_eq!(out[1].content, "hi");
    }
}
