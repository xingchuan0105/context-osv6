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
pub struct PromptLeakGuard {
    sources: Vec<(String, String)>,
}

impl PromptLeakGuard {
    pub fn new() -> Self {
        Self {
            sources: PROMPT_SOURCES
                .iter()
                .map(|(name, content)| (name.to_string(), content.to_string()))
                .collect(),
        }
    }

    /// C7: build with an explicit fingerprint source set (e.g. scanned from
    /// the runtime prompt dirs at bootstrap — see
    /// [`load_prompt_sources_from_dirs`]).
    pub fn with_sources(sources: Vec<(String, String)>) -> Self {
        Self { sources }
    }

    pub fn check(&self, response: &str, trace_id: Option<String>) -> GuardResult {
        for (name, prompt_text) in &self.sources {
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
        // C7: fenced code blocks taught to the model (e.g. ```json
        // {"skill_request": …}``` examples) must NOT become fingerprints —
        // the model emitting its own taught output is not a prompt leak.
        // Strip fences first; unfenced prose keeps the existing rules
        // (including the single-sentence single-hit rule, which is the
        // primary detection path for long Chinese sentences).
        let paragraph = strip_fence_blocks(paragraph);
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

/// Remove markdown fenced code blocks (``` ... ```) from a fingerprint
/// paragraph (C7). An unterminated fence strips to the end of the paragraph.
fn strip_fence_blocks(paragraph: &str) -> String {
    const FENCE: &str = "```";
    let mut out = String::with_capacity(paragraph.len());
    let mut rest = paragraph;
    while let Some(start) = rest.find(FENCE) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + FENCE.len()..];
        if let Some(end) = after_open.find(FENCE) {
            rest = &after_open[end + FENCE.len()..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// Load fingerprint sources by scanning prompt directories on disk (C7):
/// every `*.md` file under each dir (recursive) becomes one source named by
/// its path relative to that dir. Runtime prompts are served from these same
/// paths (`load_system_prompt` reads them), so the fingerprint set follows
/// file additions / removals / renames automatically.
pub fn load_prompt_sources_from_dirs(dirs: &[&std::path::Path]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in dirs {
        collect_markdown(dir, dir, &mut out);
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect_markdown(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(root, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let name = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                out.push((name, content));
            }
        }
    }
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

    #[test]
    fn fenced_taught_example_is_not_blocked() {
        // C7 regression (q005 class): the model emits exactly the fenced
        // skill_request example its prompt taught — that is taught output,
        // not a prompt leak. Fence blocks are stripped from fingerprints.
        let guard = PromptLeakGuard::with_sources(vec![(
            "toy-prompt".to_string(),
            "需要更多上下文时输出技能请求，格式：\n\n```json\n{\"skill_request\":[\"memory\"]}\n```\n"
                .to_string(),
        )]);
        let result = guard.check("{\"skill_request\":[\"memory\"]}", None);
        assert!(result.passed);
    }

    #[test]
    fn unfenced_single_sentence_leak_still_blocked() {
        // C7 rule choice documented: the single-sentence single-hit rule is
        // KEPT for unfenced prose (it is the primary detection path for long
        // Chinese sentences, which contain no ASCII '.'); only fenced content
        // is excluded.
        let guard = PromptLeakGuard::with_sources(vec![(
            "toy-prompt".to_string(),
            "系统提示要求：你当前 **已启用** 工作区文档检索功能".to_string(),
        )]);
        let result = guard.check("系统提示要求：你当前 **已启用** 工作区文档检索功能", None);
        assert!(!result.passed);
    }

    #[test]
    fn strip_fence_blocks_removes_only_fenced_regions() {
        let mixed = "介绍。\n\n```json\n{\"a\": 1}\n```\n\n更多说明文字在这里保留";
        let stripped = strip_fence_blocks(mixed);
        assert!(stripped.contains("介绍。"));
        assert!(stripped.contains("更多说明文字在这里保留"));
        assert!(!stripped.contains("{\"a\": 1}"));
        // Pure fenced paragraph → nothing left to fingerprint.
        let pure = "```json\n{\"a\": 1}\n```";
        assert!(strip_fence_blocks(pure).len() < MIN_PARAGRAPH_LEN);
        // Unterminated fence strips to end.
        assert!(!strip_fence_blocks("正文 ```json\n{\"a\": 1}").contains("{\"a\": 1}"));
    }

    #[test]
    fn load_prompt_sources_from_dirs_scans_recursively() {
        let dir = std::env::temp_dir().join(format!("leak_src_{}", std::process::id()));
        let sub = dir.join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.md"), "alpha").unwrap();
        std::fs::write(sub.join("b.md"), "beta").unwrap();
        std::fs::write(dir.join("ignore.txt"), "gamma").unwrap();
        let sources = load_prompt_sources_from_dirs(&[dir.as_path()]);
        let names: Vec<&str> = sources.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(sources.len(), 2, "{names:?}");
        assert!(names.iter().any(|n| n.ends_with("a.md")));
        assert!(names.iter().any(|n| n.ends_with("b.md")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
