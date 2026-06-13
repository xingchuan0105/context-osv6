use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlDetail },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageUrlDetail {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multimodal_content: Option<Vec<ContentPart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    /// Chain-of-thought from thinking-mode models (e.g. DeepSeek); must be
    /// echoed back on subsequent turns when thinking is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            multimodal_content: None,
            name: None,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            multimodal_content: None,
            name: None,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            multimodal_content: None,
            name: None,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn user_multimodal(text: impl Into<String>, image_urls: Vec<String>) -> Self {
        let mut parts = vec![ContentPart::Text { text: text.into() }];
        for url in image_urls {
            parts.push(ContentPart::ImageUrl {
                image_url: ImageUrlDetail {
                    url,
                    detail: Some("low".to_string()),
                },
            });
        }
        Self {
            role: "user".to_string(),
            content: String::new(),
            multimodal_content: Some(parts),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    /// Reasoning tokens from thinking-mode responses; carry into the next
    /// assistant message when multi-turn tool calling continues.
    pub reasoning_content: Option<String>,
    pub usage: LlmUsage,
    pub model: String,
    pub tool_calls: Option<Vec<contracts::ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// Tokens served from prompt cache (when provider supports it).
    #[serde(default)]
    pub cached_tokens: u32,
}

impl LlmUsage {
    pub fn zeroed() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            provider: String::new(),
            model: String::new(),
            cached_tokens: 0,
        }
    }

    pub fn accumulate(&mut self, other: &LlmUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.cached_tokens += other.cached_tokens;
        if self.provider.is_empty() && !other.provider.is_empty() {
            self.provider = other.provider.clone();
        }
        if self.model.is_empty() && !other.model.is_empty() {
            self.model = other.model.clone();
        }
    }
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
