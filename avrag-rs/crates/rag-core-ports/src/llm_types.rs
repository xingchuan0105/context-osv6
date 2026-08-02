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
    /// Reasoning/thinking tokens split out of `completion_tokens` (0 when the
    /// provider does not report the split).
    #[serde(default)]
    pub reasoning_tokens: u32,
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
            reasoning_tokens: 0,
        }
    }

    pub fn accumulate(&mut self, other: &LlmUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
        self.cached_tokens += other.cached_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        if self.provider.is_empty() && !other.provider.is_empty() {
            self.provider = other.provider.clone();
        }
        if self.model.is_empty() && !other.model.is_empty() {
            self.model = other.model.clone();
        }
    }

    /// Tokens charged against the loop's per-run token budget.
    ///
    /// The assembled system prefix (agent-base + capability contracts +
    /// mandatory skills) is re-sent every retrieve round; providers serve the
    /// unchanged prefix from their prompt cache (DeepSeek
    /// `prompt_cache_hit_tokens`, OpenAI `prompt_tokens_details.cached_tokens`)
    /// at a much lower cache-hit rate. Charging the cached prefix against the
    /// round budget conflates "prompt weight" with "retrieval progress" — a
    /// heavier system prompt would silently cost rounds. Billable therefore
    /// counts only uncached tokens (≈ completion + new history/observations).
    pub fn billable_tokens(&self) -> u32 {
        self.total_tokens.saturating_sub(self.cached_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billable_tokens_exclude_cached_prefix() {
        let mut u = LlmUsage::zeroed();
        u.accumulate(&LlmUsage {
            prompt_tokens: 10_000,
            completion_tokens: 300,
            total_tokens: 10_300,
            provider: "deepseek".into(),
            model: "m".into(),
            cached_tokens: 8_000,
            reasoning_tokens: 0,
        });
        assert_eq!(u.billable_tokens(), 2_300);
        // Accumulation keeps the invariant across rounds.
        u.accumulate(&LlmUsage {
            prompt_tokens: 11_000,
            completion_tokens: 400,
            total_tokens: 11_400,
            provider: String::new(),
            model: String::new(),
            cached_tokens: 8_500,
            reasoning_tokens: 0,
        });
        assert_eq!(u.billable_tokens(), 21_700 - 16_500);
        // No cache reported → billable equals total (legacy behavior).
        let plain = LlmUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            provider: String::new(),
            model: String::new(),
            cached_tokens: 0,
            reasoning_tokens: 0,
        };
        assert_eq!(plain.billable_tokens(), 150);
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
