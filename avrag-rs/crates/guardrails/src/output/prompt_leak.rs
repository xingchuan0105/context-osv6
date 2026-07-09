//! Prompt leak detection guard.

use contracts::chat::{GuardResult, RiskLevel};

const PROMPT_SOURCES: &[(&str, &str)] = &[
    (
        "rag-system",
        include_str!("../../../../prompts/orchestrators/rag-system.md"),
    ),
    (
        "search-system",
        include_str!("../../../../prompts/orchestrators/search-system.md"),
    ),
    (
        "chat-system",
        include_str!("../../../../prompts/orchestrators/chat-system.md"),
    ),
    (
        "codegen",
        include_str!("../../../../prompts/clusters/codegen/SKILL.md"),
    ),
    (
        "writing",
        include_str!("../../../../prompts/clusters/writing/SKILL.md"),
    ),
    (
        "format",
        include_str!("../../../../prompts/clusters/format/SKILL.md"),
    ),
    (
        "user-profile-extraction",
        include_str!("../../../../prompts/pipeline/user-profile-extraction.system.md"),
    ),
    (
        "triplet-extraction",
        include_str!("../../../../prompts/pipeline/triplet-extraction.system.md"),
    ),
    (
        "chat",
        include_str!("../../../../prompts/synthesis/chat.md"),
    ),
    (
        "rag-answer",
        include_str!("../../../../prompts/synthesis/rag-answer.md"),
    ),
    (
        "search-answer",
        include_str!("../../../../prompts/synthesis/search-answer.md"),
    ),
    (
        "summary_generation",
        include_str!("../../../../prompts/pipeline/summary-generation.system.v1.md"),
    ),
    (
        "summary_generation_finalize",
        include_str!("../../../../prompts/pipeline/summary-generation-finalize.system.v1.md"),
    ),
    (
        "section_index",
        include_str!("../../../../prompts/pipeline/section-index.system.v1.md"),
    ),
];

const MIN_SENTENCE_LEN: usize = 15;
const MIN_HITS_PER_PARAGRAPH: usize = 2;

/// Minimum paragraph length to be considered for leak detection.
const MIN_PARAGRAPH_LEN: usize = 30;

/// Guard that detects system prompt leakage in model output.
#[derive(Debug, Clone)]
pub struct PromptLeakGuard;

impl PromptLeakGuard {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self, response: &str, trace_id: Option<String>) -> GuardResult {
        for (name, prompt_text) in PROMPT_SOURCES {
            if let Some(leaked_paragraph) = detect_leak(response, prompt_text) {
                // Take up to 40 chars (not bytes) for the preview; slicing by byte
                // index would panic on a multibyte UTF-8 boundary (e.g. CJK text).
                let preview: String = leaked_paragraph.chars().take(40).collect();
                return GuardResult::block(
                    "output:prompt_leak",
                    RiskLevel::High,
                    format!(
                        "System prompt '{}' may have leaked: paragraph starting with '{}'...",
                        name, preview
                    ),
                    trace_id,
                    None,
                );
            }
        }
        GuardResult::pass("output:prompt_leak")
    }
}

fn detect_leak(output: &str, prompt: &str) -> Option<String> {
    for paragraph in prompt.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.len() < MIN_PARAGRAPH_LEN {
            continue;
        }

        let sentences: Vec<&str> = paragraph
            .split(['.', '?', '!'])
            .map(|s| s.trim())
            .filter(|s| s.len() >= MIN_SENTENCE_LEN)
            .collect();

        let hits = sentences.iter().filter(|s| output.contains(**s)).count();

        if sentences.len() >= MIN_HITS_PER_PARAGRAPH && hits >= MIN_HITS_PER_PARAGRAPH {
            return Some(paragraph.to_string());
        }
        if sentences.len() == 1 && hits == 1 {
            return Some(paragraph.to_string());
        }
    }
    None
}

impl Default for PromptLeakGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_response_passes() {
        let guard = PromptLeakGuard::new();
        let result = guard.check("The capital of France is Paris.", None);
        assert!(result.passed);
    }

    /// NOTE: this fixture mirrors the current `prompts/orchestrators/rag-system.md`
    /// wording (minimal v0). If that prompt is rewritten, update the leaked text
    /// below to verbatim-copy a current paragraph, otherwise the detector will
    /// correctly miss it and this test will rot (see git history of this hunk).
    #[test]
    fn paragraph_leak_is_blocked() {
        let guard = PromptLeakGuard::new();
        let leaked = "系统提示要求：你是 **RAG agent**：只根据工作区文档（经检索得到的 chunks）回答用户。事实性结论必须有检索证据支撑；证据中没有的内容不要当作文档事实写出。";
        let result = guard.check(leaked, None);
        assert!(!result.passed);
        assert_eq!(result.guard_type, "output:prompt_leak");
    }
}
