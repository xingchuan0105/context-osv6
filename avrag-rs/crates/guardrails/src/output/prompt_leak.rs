//! Prompt leak detection guard.
//!
//! Detects whether the LLM output contains fragments of system prompts
//! that were sent to it. All system prompts are loaded at compile time
//! from the prompts/ directory plus hardcoded strings used in agent code.
//!
//! Detection strategy: each system prompt is split into natural paragraphs.
//! A paragraph leak is detected when at least 2 sentences from the same
//! original paragraph appear in the output. This avoids false positives
//! from isolated technical terms that a user might naturally use.

use common::{GuardResult, RiskLevel};

/// System prompt sources loaded at compile time.
/// Each tuple is (name, full_prompt_text).
const PROMPT_SOURCES: &[(&str, &str)] = &[
    // Skills (from prompts/skills/ directory)
    ("rag-plan", include_str!("../../../../prompts/skills/rag-plan/SKILL.md")),
    (
        "rag-eval",
        include_str!("../../../../prompts/skills/rag-eval/SKILL.md"),
    ),
    (
        "search-eval",
        include_str!("../../../../prompts/skills/search-eval/SKILL.md"),
    ),
    (
        "session-summary",
        include_str!("../../../../prompts/skills/session-summary/SKILL.md"),
    ),
    (
        "user-profile-extraction",
        include_str!("../../../../prompts/skills/user-profile-extraction/SKILL.md"),
    ),
    (
        "triplet-extraction",
        include_str!("../../../../prompts/skills/triplet-extraction/SKILL.md"),
    ),
    (
        "chat",
        include_str!("../../../../prompts/skills/chat/SKILL.md"),
    ),
    (
        "rag-answer",
        include_str!("../../../../prompts/skills/rag-answer/SKILL.md"),
    ),
    (
        "search-answer",
        include_str!("../../../../prompts/skills/search-answer/SKILL.md"),
    ),
    // Templates (from prompts/ root directory)
    (
        "summary_generation",
        include_str!("../../../../prompts/summary_generation.v1.tmpl"),
    ),
    (
        "summary_generation_finalize",
        include_str!("../../../../prompts/summary_generation_finalize.v1.tmpl"),
    ),
    (
        "legacy_planner",
        include_str!("../../../../prompts/rag_planner_system.txt"),
    ),
];

/// Minimum sentence length to be considered a meaningful checkpoint.
const MIN_SENTENCE_LEN: usize = 15;

/// Minimum hits required within a paragraph to trigger leak detection.
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

    /// Check whether the response contains fragments of any system prompt.
    ///
    /// Returns a blocking `GuardResult` when leakage is detected.
    pub fn check(&self, response: &str, trace_id: Option<String>) -> GuardResult {
        for (name, prompt_text) in PROMPT_SOURCES {
            if let Some(leaked_paragraph) = detect_leak(response, prompt_text) {
                let preview_len = leaked_paragraph.len().min(40);
                return GuardResult::block(
                    "output:prompt_leak",
                    RiskLevel::High,
                    format!(
                        "System prompt '{}' may have leaked: paragraph starting with '{}'...",
                        name,
                        &leaked_paragraph[..preview_len]
                    ),
                    trace_id,
                    None,
                );
            }
        }
        GuardResult::pass("output:prompt_leak")
    }
}

/// Detect whether the output contains fragments of a given prompt.
///
/// Splits the prompt into paragraphs, then checks each paragraph for
/// multiple sentence hits within the output.
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

        // Multi-sentence paragraph: require at least MIN_HITS_PER_PARAGRAPH hits
        if sentences.len() >= MIN_HITS_PER_PARAGRAPH && hits >= MIN_HITS_PER_PARAGRAPH {
            return Some(paragraph.to_string());
        }
        // Single-sentence paragraph: require full match
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

    #[test]
    fn user_discussing_rag_tools_passes() {
        // User might naturally mention these terms in a technical discussion
        let guard = PromptLeakGuard::new();
        let result = guard.check(
            "I want to design a RAG system with dense_retrieval and graph_retrieval",
            None,
        );
        assert!(
            result.passed,
            "Isolated tool names should not trigger leak detection"
        );
    }

    #[test]
    fn paragraph_leak_is_blocked() {
        let guard = PromptLeakGuard::new();
        let leaked = "You are the Context OS RAG retrieval planner. Your job is to decide which tools should be called.";
        let result = guard.check(leaked, None);
        assert!(!result.passed);
        assert_eq!(result.guard_type, "output:prompt_leak");
    }

    #[test]
    fn single_sentence_match_passes_by_design() {
        // Design intent: MIN_HITS_PER_PARAGRAPH = 2. A single sentence echo
        // is not enough to trigger leak detection — this avoids false
        // positives when a user query or model answer happens to repeat
        // one stock phrase from a system prompt.
        let guard = PromptLeakGuard::new();
        let result = guard.check("You are a grounded answer agent.", None);
        assert!(result.passed);
    }

    #[test]
    fn partial_single_hit_passes() {
        // Only one sentence from a multi-sentence paragraph — not enough to trigger
        let guard = PromptLeakGuard::new();
        let result = guard.check(
            "Your job is to decide which tools should be called to retrieve evidence",
            None,
        );
        assert!(
            result.passed,
            "Single sentence from multi-sentence paragraph should not trigger"
        );
    }
}
