//! Exit-point usage metering hooks.
//!
//! `llm` only defines the trait and record types. Product adapters (e.g. PG)
//! live in `app-billing` and are injected via `with_observer`.

use async_trait::async_trait;
use uuid::Uuid;

/// Tenant identity attached to metered LLM / embedding calls.
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub owner_user_id: Uuid,
    pub user_id: Uuid,
}

/// Actual chat-completion usage returned by a provider.
#[derive(Debug, Clone)]
pub struct ChatUsageRecord {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// Provider prompt-cache hit tokens (0 when unknown).
    pub cached_tokens: u32,
    pub provider: String,
    pub model: String,
    /// Product feature label (e.g. `agent_loop`, `write:refine`, `summary`).
    pub feature: String,
    /// Optional stage / mode label (e.g. `chat`, `rag`, `worker_summary`).
    pub stage: String,
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Embedding usage at the API exit point.
#[derive(Debug, Clone)]
pub struct EmbeddingUsageRecord {
    pub estimated_tokens: u32,
    /// Present when the provider returns actual token usage (e.g. DashScope MM).
    pub actual_tokens: Option<u32>,
    pub provider: String,
    pub model: String,
    pub feature: String,
}

/// Fail-open metering observer: implementors must not fail the LLM path.
#[async_trait]
pub trait UsageObserver: Send + Sync {
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord);

    async fn record_embedding(&self, tenant: &TenantContext, record: &EmbeddingUsageRecord);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct CaptureObserver {
        chats: Mutex<Vec<ChatUsageRecord>>,
        embeds: Mutex<Vec<EmbeddingUsageRecord>>,
    }

    #[async_trait]
    impl UsageObserver for CaptureObserver {
        async fn record_chat(&self, _tenant: &TenantContext, record: &ChatUsageRecord) {
            self.chats.lock().unwrap().push(record.clone());
        }

        async fn record_embedding(&self, _tenant: &TenantContext, record: &EmbeddingUsageRecord) {
            self.embeds.lock().unwrap().push(record.clone());
        }
    }

    #[tokio::test]
    async fn capture_observer_records_chat_fields() {
        let obs = Arc::new(CaptureObserver {
            chats: Mutex::new(Vec::new()),
            embeds: Mutex::new(Vec::new()),
        });
        let tenant = TenantContext {
            owner_user_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
        };
        let record = ChatUsageRecord {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            cached_tokens: 3,
            provider: "openai".into(),
            model: "gpt-test".into(),
            feature: "agent_loop".into(),
            stage: "chat".into(),
            session_id: None,
            document_id: None,
            request_id: Some("req-1".into()),
            trace_id: None,
        };
        obs.record_chat(&tenant, &record).await;
        let chats = obs.chats.lock().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].total_tokens, 15);
        assert_eq!(chats[0].feature, "agent_loop");
    }
}
