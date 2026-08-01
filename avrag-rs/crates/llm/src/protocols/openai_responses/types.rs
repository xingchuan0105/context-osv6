//! Shared types for the OpenAI Responses protocol (DeepSeek `/v1/responses`).
use crate::schema::{LlmUsage, Usage};
use serde::Deserialize;

pub(crate) const TEXT_BLOCK_ID: &str = "text-0";
pub(crate) const REASONING_BLOCK_ID: &str = "reasoning-0";

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiResponsesProtocol;

/// In-flight accumulator for one streaming function call, keyed by its
/// `output_index` in the Responses stream.
#[derive(Debug, Default)]
pub(crate) struct ToolCallAcc {
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Debug, Default)]
pub struct OpenAiResponsesState {
    pub(crate) accumulated_content: String,
    pub(crate) accumulated_reasoning: String,
    pub(crate) usage: Option<LlmUsage>,
    pub(crate) model: String,
    pub(crate) provider: String,
    /// Kept for request/response correlation and future routing.
    #[allow(dead_code)]
    pub(crate) configured_model: String,
    pub(crate) tool_calls: Option<Vec<contracts::ToolCall>>,
    pub(crate) tool_accumulators: Vec<ToolCallAcc>,
    pub(crate) text_started: bool,
    pub(crate) reasoning_started: bool,
    pub(crate) failed_message: Option<String>,
    pub(crate) incomplete: bool,
}

/// Provider usage block for the Responses protocol.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct ResponsesUsageRaw {
    #[serde(default)]
    pub(crate) input_tokens: u32,
    #[serde(default)]
    pub(crate) output_tokens: u32,
    #[serde(default)]
    pub(crate) total_tokens: u32,
    #[serde(default)]
    pub(crate) input_tokens_details: Option<ResponsesTokensDetails>,
    #[serde(default)]
    /// Reasoning token split (not consumed by usage metering yet).
    #[allow(dead_code)]
    pub(crate) output_tokens_details: Option<ResponsesTokensDetails>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ResponsesTokensDetails {
    #[serde(default)]
    pub(crate) cached_tokens: u32,
    #[serde(default)]
    /// Split of reasoning tokens inside `output_tokens`; parsed for future
    /// reasoning-cost accounting.
    #[allow(dead_code)]
    pub(crate) reasoning_tokens: u32,
}

impl ResponsesUsageRaw {
    pub(crate) fn cached_token_count(&self) -> u32 {
        self.input_tokens_details
            .as_ref()
            .map(|d| d.cached_tokens)
            .unwrap_or(0)
    }

    pub(crate) fn to_llm_usage(&self, provider: String, model: String) -> LlmUsage {
        LlmUsage {
            prompt_tokens: self.input_tokens,
            completion_tokens: self.output_tokens,
            total_tokens: self.total_tokens,
            provider,
            model,
            cached_tokens: self.cached_token_count(),
        }
    }
}

/// One item in the Responses `output` array.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponsesOutputItem {
    #[serde(rename = "type")]
    pub(crate) item_type: String,
    #[serde(default)]
    /// Message items carry a role; kept for completeness of the wire model.
    #[allow(dead_code)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) content: Option<Vec<ResponsesContentPart>>,
    #[serde(default)]
    pub(crate) call_id: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) arguments: Option<String>,
    /// Reasoning item summary (DeepSeek does not generate it, OpenAI does).
    #[serde(default)]
    pub(crate) summary: Option<Vec<ResponsesContentPart>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResponsesContentPart {
    #[serde(rename = "type")]
    pub(crate) part_type: String,
    #[serde(default)]
    pub(crate) text: Option<String>,
}

/// Top-level Responses object: the non-streaming response body, and the
/// payload carried by the terminal `response.completed` / `response.incomplete`
/// / `response.failed` stream events.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponsesObject {
    #[serde(default)]
    pub(crate) output: Vec<ResponsesOutputItem>,
    #[serde(default)]
    pub(crate) usage: Option<ResponsesUsageRaw>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) error: Option<ResponsesError>,
    #[serde(default)]
    pub(crate) model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResponsesError {
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    /// Machine-stable error code; surfaced via the message only for now.
    #[allow(dead_code)]
    pub(crate) code: Option<String>,
}

pub(crate) fn usage_to_event_usage(usage: &LlmUsage) -> Usage {
    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
    }
}
