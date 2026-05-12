//! Tool specification types for agent tool-use loop.
//!
//! These types define the contract between the LLM (which decides which tools
//! to call) and the agent runtime (which executes them).

use serde::{Deserialize, Serialize};

/// Tool specification exposed to the model.
///
/// Follows the OpenAI function-calling schema so that providers which
/// support native tool-calling can pass this through directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model decided it was done answering.
    EndTurn,
    /// Model hit a custom stop sequence.
    StopSequence,
    /// Model wants to call one or more tools.
    ToolUse,
    /// Model ran out of context window.
    MaxTokens,
}

/// A single tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelToolCall {
    /// Provider-assigned call ID (used to match results back to the call).
    pub id: String,
    /// Tool name — must match a registered `ToolSpec::name`.
    pub name: String,
    /// Arguments as a JSON object.
    pub arguments: serde_json::Value,
}

/// Response from a model that supports tool calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAwareResponse {
    /// Text content produced by the model (may be empty if it went straight to ToolUse).
    pub content: String,
    /// Tool calls the model wants to make.
    pub tool_calls: Vec<ModelToolCall>,
    /// Why the model stopped.
    pub stop_reason: StopReason,
}
