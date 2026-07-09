//! LLM-adjacent DTOs shared across rag-core without depending on avrag-llm.

use contracts::chat::AnswerBlock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
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

/// Synthesized answer payload consumed by rag-core response assembly.
#[derive(Debug, Clone, Default)]
pub struct SynthesisOutput {
    pub answer_text: String,
    pub answer_blocks: Vec<AnswerBlock>,
    pub cited_chunk_ids: Vec<String>,
    pub llm_usage: Option<LlmUsage>,
}
