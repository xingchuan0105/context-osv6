//! Recording wrapper around LlmProvider for capturing prompts and responses.

use avrag_llm::{ChatMessage, LlmProvider, LlmResponse};
use std::sync::{Arc, Mutex};

/// A recorded LLM call with extracted system prompt and response.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LlmCall {
    pub system_prompt: String,
    pub user_messages: Vec<ChatMessage>,
    pub response_content: String,
    pub timestamp_ms: u64,
}

/// Wraps a real LlmProvider, recording every `complete()` call before delegating.
pub struct RecordingLlmProvider {
    inner: Arc<dyn LlmProvider>,
    calls: Arc<Mutex<Vec<LlmCall>>>,
}

impl RecordingLlmProvider {
    pub fn new(inner: Arc<dyn LlmProvider>) -> Self {
        Self {
            inner,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn calls(&self) -> Vec<LlmCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl LlmProvider for RecordingLlmProvider {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        // Extract system prompt (first message with role "system")
        let system_prompt = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Extract user messages
        let user_messages: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        // Delegate to real provider
        let response = self.inner.complete(messages, temperature).await?;

        // Record the call
        let call = LlmCall {
            system_prompt,
            user_messages,
            response_content: response.content.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        self.calls.lock().unwrap().push(call);

        Ok(response)
    }

    async fn complete_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[common::ToolSpec],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        // Extract system prompt (first message with role "system")
        let system_prompt = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Extract user messages
        let user_messages: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        // Delegate to real provider
        let response = self
            .inner
            .complete_with_tools(messages, tools, temperature)
            .await?;

        // Record the call
        let call = LlmCall {
            system_prompt,
            user_messages,
            response_content: response.content.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        self.calls.lock().unwrap().push(call);

        Ok(response)
    }
}
