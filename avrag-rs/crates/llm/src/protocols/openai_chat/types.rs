//! Shared types for the OpenAI chat-completions protocol.
use crate::schema::{LlmUsage, Usage};
use serde::Deserialize;

pub(crate) const TEXT_BLOCK_ID: &str = "text-0";
pub(crate) const REASONING_BLOCK_ID: &str = "reasoning-0";

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiChatProtocol;

#[derive(Debug, Default)]
pub struct OpenAiChatState {
    pub(crate) accumulated_content: String,
    pub(crate) accumulated_reasoning: String,
    pub(crate) usage: Option<LlmUsage>,
    pub(crate) model: String,
    pub(crate) provider: String,
    pub(crate) configured_model: String,
    pub(crate) tool_calls: Option<Vec<contracts::ToolCall>>,
    pub(crate) text_started: bool,
    pub(crate) reasoning_started: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChoiceDelta {
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) call_type: String,
    #[serde(default)]
    pub(crate) function: Option<OpenAiFunctionCall>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiFunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompletionChoiceMessage {
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) reasoning_content: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChoice {
    #[serde(default)]
    pub(crate) delta: Option<StreamChoiceDelta>,
    #[serde(default)]
    pub(crate) message: Option<CompletionChoiceMessage>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct PromptTokensDetails {
    #[serde(default)]
    pub(crate) cached_tokens: u32,
}

/// Provider usage block (OpenAI-compatible + DeepSeek cache fields).
#[derive(Debug, Deserialize, Default)]
pub(crate) struct ApiUsageRaw {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
    #[serde(default)]
    cached_tokens: u32,
    #[serde(default)]
    prompt_cache_hit_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

impl ApiUsageRaw {
    pub(crate) fn from_token_counts(
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
        cached_tokens: u32,
    ) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_tokens,
            prompt_cache_hit_tokens: 0,
            prompt_tokens_details: None,
        }
    }

    pub(crate) fn prompt_tokens(&self) -> u32 {
        self.prompt_tokens
    }

    pub(crate) fn completion_tokens(&self) -> u32 {
        self.completion_tokens
    }

    pub(crate) fn total_tokens(&self) -> u32 {
        self.total_tokens
    }

    pub(crate) fn cached_token_count(&self) -> u32 {
        if self.cached_tokens > 0 {
            self.cached_tokens
        } else if self.prompt_cache_hit_tokens > 0 {
            self.prompt_cache_hit_tokens
        } else {
            self.prompt_tokens_details
                .as_ref()
                .map(|d| d.cached_tokens)
                .unwrap_or(0)
        }
    }

    pub(crate) fn to_llm_usage(&self, provider: String, model: String) -> LlmUsage {
        LlmUsage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
            provider,
            model,
            cached_tokens: self.cached_token_count(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChunk {
    #[serde(default)]
    pub(crate) choices: Vec<StreamChoice>,
    #[serde(default)]
    pub(crate) usage: Option<ApiUsageRaw>,
    #[serde(default)]
    pub(crate) model: Option<String>,
}

pub(crate) fn map_openai_tool_calls(calls: &[OpenAiToolCall]) -> Option<Vec<contracts::ToolCall>> {
    let mut mapped_calls = Vec::new();
    for tool_call in calls {
        if let Some(ref func) = tool_call.function {
            let args = serde_json::from_str(&func.arguments)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            mapped_calls.push(contracts::ToolCall {
                tool: func.name.clone(),
                version: "1.0".to_string(),
                args,
            });
        }
    }
    if mapped_calls.is_empty() {
        None
    } else {
        Some(mapped_calls)
    }
}

pub(crate) fn apply_message_to_accumulators(
    message: &CompletionChoiceMessage,
    accumulated_content: &mut String,
    accumulated_reasoning: &mut String,
    on_content_delta: &mut impl FnMut(&str),
    on_reasoning_delta: &mut impl FnMut(&str),
) {
    if let Some(reasoning) = message.reasoning_content.as_deref() {
        if !reasoning.is_empty() {
            accumulated_reasoning.push_str(reasoning);
            on_reasoning_delta(reasoning);
        }
    }
    if let Some(content) = message.content.as_deref() {
        if !content.is_empty() {
            accumulated_content.push_str(content);
            on_content_delta(content);
        }
    }
}

pub(crate) fn apply_delta_to_accumulators(
    delta: &StreamChoiceDelta,
    accumulated_content: &mut String,
    accumulated_reasoning: &mut String,
    on_content_delta: &mut impl FnMut(&str),
    on_reasoning_delta: &mut impl FnMut(&str),
) {
    if let Some(reasoning) = delta.reasoning_content.as_deref() {
        if !reasoning.is_empty() {
            accumulated_reasoning.push_str(reasoning);
            on_reasoning_delta(reasoning);
        }
    }
    if let Some(content) = delta.content.as_deref() {
        if !content.is_empty() {
            accumulated_content.push_str(content);
            on_content_delta(content);
        }
    }
}

pub(crate) fn usage_to_event_usage(usage: &LlmUsage) -> Usage {
    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
    }
}

