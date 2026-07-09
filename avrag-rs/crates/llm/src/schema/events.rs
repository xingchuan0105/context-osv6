use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cached_tokens: u32,
}

/// Shared with rag-core via ports (single definition).
pub use avrag_rag_core_ports::LlmUsage;

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub reasoning_content: Option<String>,
    pub usage: LlmUsage,
    pub model: String,
    pub tool_calls: Option<Vec<contracts::ToolCall>>,
}

impl LlmResponse {
    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn reasoning(&self) -> Option<&str> {
        self.reasoning_content.as_deref()
    }

    pub fn tool_calls(&self) -> Option<&[contracts::ToolCall]> {
        self.tool_calls.as_deref()
    }
}

#[derive(Debug, Clone)]
pub enum LlmEvent {
    StepStart { index: usize },
    TextStart { id: String },
    TextDelta { id: String, text: String },
    TextEnd { id: String },
    ReasoningStart { id: String },
    ReasoningDelta { id: String, text: String },
    ReasoningEnd { id: String },
    ToolInputStart { id: String, name: String },
    ToolInputDelta { id: String, name: String, text: String },
    ToolInputEnd { id: String, name: String },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        id: String,
        name: String,
        result: serde_json::Value,
    },
    ToolError {
        id: String,
        name: String,
        message: String,
    },
    StepFinish {
        index: usize,
        reason: FinishReason,
        usage: Option<Usage>,
    },
    Finish {
        reason: FinishReason,
        usage: Option<Usage>,
    },
    ProviderError {
        message: String,
        retryable: Option<bool>,
    },
}

#[cfg(test)]
mod tests {
    use super::LlmUsage;

    #[test]
    fn llm_usage_accumulate_preserves_provider_and_model() {
        let mut total = LlmUsage::zeroed();
        total.accumulate(&LlmUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            provider: "dmxapi".to_string(),
            model: "gemini-test".to_string(),
            cached_tokens: 5,
        });

        assert_eq!(total.total_tokens, 30);
        assert_eq!(total.provider, "dmxapi");
        assert_eq!(total.model, "gemini-test");
    }
}
