use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient, LlmUsage};

/// Session-wide guidance shared by every ingestion turn. Authored under
/// `prompts/pipeline/`, never inline.
pub(crate) const INTERACTION_SESSION_SYSTEM: &str =
    include_str!("../../../../prompts/pipeline/interaction-session.system.md");

/// Result of one session turn (seed or follow-up).
#[derive(Debug, Clone)]
pub(crate) struct SessionTurn {
    pub(crate) content: String,
    pub(crate) usage: LlmUsage,
}

/// 一轮会话的消息组装（纯函数，可测）。
///
/// **DashScope 会话缓存键包含 `instructions`（system 消息）：续接轮必须与
/// seed 轮 instructions 完全一致才命中**（2026-08-03 真机 A/B：同 → cached
/// 2217；异/无 → 0）。因此 instructions 恒定为 `INTERACTION_SESSION_SYSTEM`，
/// 阶段 system prompt（section-index/summary/triplet）下沉为 user 消息的前导
/// 块——否则三阶段各不相同，缓存永远不命中（线上实测后果：每轮全价，
/// prompt≈全部上下文，cached=0）。
fn build_turn_messages(stage_prompt: &str, user: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(INTERACTION_SESSION_SYSTEM),
        ChatMessage::user(format!("{stage_prompt}\n\n{user}")),
    ]
}

/// A single LLM conversation chain for one document's ingestion.
///
/// `seed` opens the chain without a `previous_response_id`; every follow-up
/// `produce` continues the same chain through the provider's session cache.
/// The session deliberately bypasses the result-level completion cache so the
/// chain stays continuous and every turn can hit the provider-side cache.
#[derive(Debug, Clone)]
pub(crate) struct DocumentIngestionSession {
    llm: Arc<LlmClient>,
    previous_response_id: Option<String>,
    total_tokens: u64,
}

impl DocumentIngestionSession {
    pub(crate) fn new(llm: Arc<LlmClient>) -> Self {
        Self {
            llm,
            previous_response_id: None,
            total_tokens: 0,
        }
    }

    /// First turn of the chain: no previous response to continue from.
    /// `stage_prompt` 为当轮阶段指令（折叠进 user 消息，见
    /// `build_turn_messages` 的缓存键说明）。
    pub(crate) async fn seed(
        &mut self,
        stage_prompt: &str,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        self.complete(None, stage_prompt, user, temperature).await
    }

    /// Follow-up turn: continues the same session chain.
    pub(crate) async fn produce(
        &mut self,
        stage_prompt: &str,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let previous_response_id = self.previous_response_id.clone();
        self.complete(previous_response_id.as_deref(), stage_prompt, user, temperature)
            .await
    }

    async fn complete(
        &mut self,
        previous_response_id: Option<&str>,
        stage_prompt: &str,
        user: &str,
        temperature: Option<f32>,
    ) -> anyhow::Result<SessionTurn> {
        let messages = build_turn_messages(stage_prompt, user);
        let (response, next_id) = self
            .llm
            .complete_response(previous_response_id, &messages, temperature)
            .await?;
        self.previous_response_id = next_id;
        self.total_tokens = self
            .total_tokens
            .saturating_add(u64::from(response.usage.total_tokens));
        Ok(SessionTurn {
            content: response.content,
            usage: response.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 缓存键约束：instructions（system 消息）恒定且只含会话级引导；
    /// 阶段指令必须折叠进 user 消息前导块。
    #[test]
    fn turn_messages_keep_instructions_constant_and_fold_stage_prompt() {
        let messages = build_turn_messages("阶段指令：输出 JSON。", "正文内容");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, INTERACTION_SESSION_SYSTEM);
        assert_eq!(messages[1].role, "user");
        assert!(messages[1].content.starts_with("阶段指令：输出 JSON。\n\n"));
        assert!(messages[1].content.ends_with("正文内容"));
        // 阶段指令不得泄漏到 system 消息（否则续接轮 instructions 变化 → 缓存不命中）
        assert!(!messages[0].content.contains("阶段指令"));
    }
}
