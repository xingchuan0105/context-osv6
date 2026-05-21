use common::ChatMessage;
use serde::{Deserialize, Serialize};

fn normalize_for_similarity(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn assistant_similarity(left: &str, right: &str) -> f32 {
    let left_tokens = normalize_for_similarity(left);
    let right_tokens = normalize_for_similarity(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let overlap = left_tokens
        .iter()
        .filter(|token| right_tokens.contains(token))
        .count();
    overlap as f32 / left_tokens.len().max(right_tokens.len()) as f32
}

pub fn dedupe_adjacent_assistant_messages(
    messages: &[ChatMessage],
    similarity_threshold: f32,
) -> Vec<ChatMessage> {
    let mut deduped = Vec::with_capacity(messages.len());

    for message in messages {
        let should_skip = message.role == "assistant"
            && deduped.last().is_some_and(|previous: &ChatMessage| {
                previous.role == "assistant"
                    && assistant_similarity(&previous.content, &message.content)
                        >= similarity_threshold
            });
        if !should_skip {
            deduped.push(message.clone());
        }
    }

    deduped
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMemoryTurn {
    pub user_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceShortTermMemoryWindow {
    #[serde(default)]
    pub turns: Vec<WorkspaceMemoryTurn>,
}

pub fn build_short_term_memory_window(
    messages: &[ChatMessage],
    max_turns: usize,
    similarity_threshold: f32,
) -> WorkspaceShortTermMemoryWindow {
    let deduped = dedupe_adjacent_assistant_messages(messages, similarity_threshold);
    let mut turns = Vec::new();
    let mut index = 0usize;

    while index < deduped.len() {
        let current = &deduped[index];
        if current.role != "user" {
            index += 1;
            continue;
        }

        let assistant_message = deduped
            .get(index + 1)
            .filter(|message| message.role == "assistant")
            .map(|message| message.content.clone());
        let has_assistant_message = assistant_message.is_some();
        turns.push(WorkspaceMemoryTurn {
            user_message: current.content.clone(),
            assistant_message,
        });
        index += if has_assistant_message { 2 } else { 1 };
    }

    let keep_from = turns.len().saturating_sub(max_turns);
    WorkspaceShortTermMemoryWindow {
        turns: turns.into_iter().skip(keep_from).collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryObject {
    pub name: String,
    #[serde(default)]
    pub facts: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceLongTermMemory {
    #[serde(default)]
    pub narrative: String,
    #[serde(default)]
    pub objects: Vec<MemoryObject>,
}

pub fn should_refresh_long_term_memory(turn_count: usize, every_n_turns: usize) -> bool {
    (6..=8).contains(&every_n_turns) && turn_count > 0 && turn_count.is_multiple_of(every_n_turns)
}

pub trait LongTermMemoryStore: Send + Sync {
    fn load(&self, workspace_id: &str) -> anyhow::Result<Option<WorkspaceLongTermMemory>>;
    fn store(&self, workspace_id: &str, memory: &WorkspaceLongTermMemory) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct NoopMemvidStore;

impl LongTermMemoryStore for NoopMemvidStore {
    fn load(&self, _workspace_id: &str) -> anyhow::Result<Option<WorkspaceLongTermMemory>> {
        Ok(None)
    }

    fn store(&self, _workspace_id: &str, _memory: &WorkspaceLongTermMemory) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat_message(id: i64, role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            id,
            session_id: "session-1".to_string(),
            role: role.to_string(),
            content: content.to_string(),
            answer_blocks: Vec::new(),
            agent_id: None,
            agent_name: None,
            agent_icon: None,
            citations: Vec::new(),
            tool_results: Vec::new(),
            created_at: "2026-04-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn dedupe_adjacent_assistant_messages_removes_similar_retries() {
        let messages = vec![
            chat_message(1, "user", "What changed?"),
            chat_message(
                2,
                "assistant",
                "The rollout failed because config A was missing.",
            ),
            chat_message(
                3,
                "assistant",
                "The rollout failed because config A was missing.",
            ),
            chat_message(4, "user", "What should we do next?"),
        ];

        let deduped = dedupe_adjacent_assistant_messages(&messages, 0.8);

        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[1].role, "assistant");
        assert_eq!(
            deduped[1].content,
            "The rollout failed because config A was missing."
        );
    }

    #[test]
    fn short_term_memory_window_keeps_recent_turns() {
        let messages = vec![
            chat_message(1, "user", "u1"),
            chat_message(2, "assistant", "a1"),
            chat_message(3, "user", "u2"),
            chat_message(4, "assistant", "a2"),
            chat_message(5, "user", "u3"),
            chat_message(6, "assistant", "a3"),
            chat_message(7, "user", "u4"),
            chat_message(8, "assistant", "a4"),
            chat_message(9, "user", "u5"),
            chat_message(10, "assistant", "a5"),
        ];

        let window = build_short_term_memory_window(&messages, 4, 0.8);

        assert_eq!(window.turns.len(), 4);
        assert_eq!(window.turns[0].user_message, "u2");
        assert_eq!(window.turns[3].assistant_message.as_deref(), Some("a5"));
    }

    #[test]
    fn long_term_memory_refresh_uses_six_to_eight_turn_policy() {
        assert!(should_refresh_long_term_memory(6, 6));
        assert!(should_refresh_long_term_memory(8, 8));
        assert!(!should_refresh_long_term_memory(5, 6));
        assert!(!should_refresh_long_term_memory(9, 9));
    }

    #[test]
    fn noop_memvid_store_is_safe_default() {
        let store = NoopMemvidStore;
        let memory = WorkspaceLongTermMemory {
            narrative: "User is tracking rollout regressions.".to_string(),
            objects: vec![MemoryObject {
                name: "atlas".to_string(),
                facts: vec!["rollback runbook".to_string()],
            }],
        };

        store.store("workspace-1", &memory).unwrap();
        assert!(store.load("workspace-1").unwrap().is_none());
    }
}
