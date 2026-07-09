use super::*;
const STALE_PROCESSING_TIMEOUT_SECS: i32 = 30 * 60;
const RETRY_BACKOFF_BASE_SECS: i32 = 30;
const RETRY_BACKOFF_MAX_SECS: i32 = 60 * 60;

pub fn retry_backoff_seconds(attempt_count: i32) -> i32 {
    let exponent = attempt_count.saturating_sub(1).clamp(0, 7) as u32;
    let seconds = RETRY_BACKOFF_BASE_SECS.saturating_mul(1_i32 << exponent);
    seconds.clamp(RETRY_BACKOFF_BASE_SECS, RETRY_BACKOFF_MAX_SECS)
}

pub fn ingestion_retry_backoff_seconds(attempt_count: i32) -> i32 {
    retry_backoff_seconds(attempt_count)
}

pub fn ingestion_queue_group_from_env() -> String {
    std::env::var("AVRAG_INGESTION_QUEUE_GROUP").unwrap_or_else(|_| "default".to_string())
}

pub struct ChatTurn<'a> {
    pub user_content: &'a str,
    pub assistant_content: &'a str,
    pub assistant_answer_blocks: &'a [contracts::chat::AnswerBlock],
    pub agent_type: &'a str,
    pub citations: &'a [contracts::chat::Citation],
    pub tool_results: &'a [contracts::ToolResult],
    /// Metadata for the user message row (e.g. query_resolution per ADR-0008).
    pub user_turn_metadata: Option<serde_json::Value>,
    /// Non-destructive resolved query for retrieval (ADR-0008); `user_content` stays raw.
    pub user_resolved_query: Option<&'a str>,
}
