use avrag_llm::ChatMessage;
use contracts::{ToolResult, ToolStatus};

use super::config::{LoopExitConfig, ModeConfig};

const RAG_EVIDENCE_TOOLS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "graph_retrieval",
    "index_lookup",
    "doc_summary",
    "doc_metadata",
    "doc_profile",
];

const SEARCH_EVIDENCE_TOOLS: &[&str] = &["web_search", "web_fetch"];

// ---------------------------------------------------------------------------
// Synthesis gate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostLoopAction {
    EnterSynthesis,
    DegradedNoEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynthesisGate {
    EnterSynthesis,
    RunFallbackThenCheck,
    SkipSynthesisUseDirect(String),
}

pub fn decide_synthesis_gate(
    loop_exit: &LoopExitConfig,
    has_evidence: bool,
    direct_answer: Option<&str>,
    _tool_results: &[ToolResult],
    _query: &str,
) -> SynthesisGate {
    if let Some(answer) = direct_answer {
        if loop_exit.skip_synthesis_on_direct_answer {
            return SynthesisGate::SkipSynthesisUseDirect(answer.to_string());
        }
    }

    if has_evidence || !loop_exit.require_evidence {
        SynthesisGate::EnterSynthesis
    } else {
        SynthesisGate::RunFallbackThenCheck
    }
}

pub fn post_fallback_gate(loop_exit: &LoopExitConfig, has_evidence: bool) -> PostLoopAction {
    decide_post_loop(loop_exit, has_evidence)
}

pub(crate) fn stdout_is_placeholder(stdout: &str) -> bool {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "[]" | "{}" | "no results" | "no result" | "none"
    )
}

/// Opening tag prefix used by codegen sandbox observations. We split on the prefix
/// (without the trailing `>`) because the opening tag may carry attributes, e.g.
/// `<code_execution_result untrusted="true">`. The closing tag remains the bare
/// `</code_execution_result>`.
const CODE_EXECUTION_RESULT_OPEN: &str = "<code_execution_result";
const CODE_EXECUTION_RESULT_CLOSE: &str = "</code_execution_result>";

/// True when `message_content` contains a (possibly attribute-bearing) code execution
/// result block, i.e. `<code_execution_result ...>...</code_execution_result>`.
fn has_code_execution_result_block(message_content: &str) -> bool {
    message_content.contains(CODE_EXECUTION_RESULT_OPEN)
        && message_content.contains(CODE_EXECUTION_RESULT_CLOSE)
}

/// Returns true when a `<code_execution_result>` observation carries retrieval output.
pub fn code_execution_has_evidence(message_content: &str) -> bool {
    // Split on the opening tag *prefix* so attribute-bearing tags
    // (e.g. `<code_execution_result untrusted="true">`) are still matched.
    let Some(inner) = message_content
        .split(CODE_EXECUTION_RESULT_OPEN)
        .nth(1)
        .and_then(|s| s.split(CODE_EXECUTION_RESULT_CLOSE).next())
    else {
        return false;
    };

    for segment in inner.split("[block ") {
        let Some(stdout_part) = segment.split_once("stdout:") else {
            continue;
        };
        let after_stdout = stdout_part.1;
        let stdout = after_stdout
            .split_once("stderr:")
            .map(|(stdout, _)| stdout)
            .unwrap_or(after_stdout);
        if !stdout_is_placeholder(stdout) {
            return true;
        }
    }
    false
}

fn chunk_array_non_empty(data: &serde_json::Value) -> bool {
    if let Some(arr) = data.as_array() {
        return !arr.is_empty();
    }
    if let Some(chunks) = data.get("chunks").and_then(|v| v.as_array()) {
        return !chunks.is_empty();
    }
    false
}

/// True when a RAG tool result carries at least one chunk/item.
pub fn tool_result_has_chunks(result: &ToolResult) -> bool {
    if result.status != ToolStatus::Ok {
        return false;
    }
    if !RAG_EVIDENCE_TOOLS.contains(&result.tool.as_str()) {
        return false;
    }
    result.data.as_ref().is_some_and(chunk_array_non_empty)
}

/// True when a Search tool result carries usable hits (non-empty results / body).
/// Empty `web_search` Ok responses must **not** count as evidence — that was
/// the main driver of search-loop idle spinning (空转).
pub fn tool_result_has_web_hits(result: &ToolResult) -> bool {
    if result.status != ToolStatus::Ok {
        return false;
    }
    match result.tool.as_str() {
        "web_search" => result
            .data
            .as_ref()
            .and_then(|d| d.get("results"))
            .and_then(|r| r.as_array())
            .is_some_and(|a| !a.is_empty()),
        "web_fetch" => result.data.as_ref().is_some_and(|d| {
            d.get("content")
                .and_then(|c| c.as_str())
                .is_some_and(|s| !s.trim().is_empty())
                || d.get("text")
                    .and_then(|c| c.as_str())
                    .is_some_and(|s| !s.trim().is_empty())
                || d.get("markdown")
                    .and_then(|c| c.as_str())
                    .is_some_and(|s| !s.trim().is_empty())
        }),
        _ => false,
    }
}

/// How many trailing search-tool results are empty/failed with no hits in the
/// whole trail (used to early-exit the search retrieve loop).
pub fn consecutive_empty_search_tail(tool_results: &[ToolResult]) -> usize {
    let mut n = 0usize;
    for r in tool_results.iter().rev() {
        if !SEARCH_EVIDENCE_TOOLS.contains(&r.tool.as_str()) {
            break;
        }
        if tool_result_has_web_hits(r) {
            break;
        }
        n += 1;
    }
    n
}

/// Stop the search retrieve loop after this many consecutive empty search
/// tool results (saves the remaining iteration budget).
pub const SEARCH_EMPTY_EARLY_STOP_THRESHOLD: usize = 2;

pub fn should_early_stop_search_on_empty(mode: &ModeConfig, tool_results: &[ToolResult]) -> bool {
    mode.id == "search"
        && consecutive_empty_search_tail(tool_results) >= SEARCH_EMPTY_EARLY_STOP_THRESHOLD
        && !tool_results.iter().any(tool_result_has_web_hits)
}

pub fn has_retrieval_observation(
    messages: &[ChatMessage],
    collected_tool_results: &[ToolResult],
    mode: &ModeConfig,
) -> bool {
    if mode.id == "rag" {
        if messages.iter().any(|m| {
            m.role == "user"
                && has_code_execution_result_block(&m.content)
                && code_execution_has_evidence(&m.content)
        }) {
            return true;
        }
        return collected_tool_results.iter().any(tool_result_has_chunks);
    }
    if mode.id == "search" {
        if collected_tool_results.iter().any(tool_result_has_web_hits) {
            return true;
        }
        // Message-content fallback: only if a URL-bearing observation is present
        // (keeps older fixtures working; empty result payloads without urls stay false).
        return messages.iter().any(|m| {
            m.content.contains("\"url\"")
                && (m.content.contains("web_search") || m.content.contains("\"results\""))
        });
    }
    true
}

pub fn should_block_content_early_stop(loop_exit: &LoopExitConfig, has_evidence: bool) -> bool {
    loop_exit.require_evidence && !has_evidence && !loop_exit.allow_content_early_stop
}

pub fn decide_post_loop(loop_exit: &LoopExitConfig, has_evidence: bool) -> PostLoopAction {
    if has_evidence || !loop_exit.require_evidence {
        PostLoopAction::EnterSynthesis
    } else {
        PostLoopAction::DegradedNoEvidence
    }
}

pub fn degraded_no_evidence_answer(mode_id: &str) -> String {
    match mode_id {
        "rag" => "I could not find relevant evidence in your documents for this question. \
                  Please try rephrasing or upload additional material."
            .to_string(),
        "search" => "I could not retrieve web evidence to answer this question. \
                      Please try again with a more specific query."
            .to_string(),
        _ => "I do not have enough information to answer this question.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rag_mode() -> ModeConfig {
        super::super::config::load_mode_config("rag").unwrap()
    }

    #[test]
    fn detects_code_execution_observation() {
        let mode = rag_mode();
        let messages = vec![ChatMessage::user(
            "<code_execution_result>\n[block 0] stdout: chunks found\nstderr: \n</code_execution_result>",
        )];
        assert!(has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn detects_code_execution_observation_with_untrusted_attribute() {
        // The opening tag may carry attributes (e.g. untrusted="true"). Parsing must still
        // match on the tag prefix, and the closing tag must remain recognized.
        let content = "<code_execution_result untrusted=\"true\">\n[block 0] stdout: chunks found\nstderr: \n</code_execution_result>";
        assert!(has_code_execution_result_block(content));
        assert!(code_execution_has_evidence(content));
        let mode = rag_mode();
        let messages = vec![ChatMessage::user(content)];
        assert!(has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn empty_stdout_stderr_is_not_evidence() {
        let content =
            "<code_execution_result>\n[block 0] stdout: \nstderr: \n</code_execution_result>";
        assert!(!code_execution_has_evidence(content));
        let mode = rag_mode();
        let messages = vec![ChatMessage::user(content)];
        assert!(!has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn stderr_only_error_is_not_evidence() {
        let content = "<code_execution_result>\n[block 0] stdout: \nstderr: NameError: x\n</code_execution_result>";
        assert!(!code_execution_has_evidence(content));
    }

    #[test]
    fn stdout_placeholder_is_not_evidence() {
        let content =
            "<code_execution_result>\n[block 0] stdout: []\nstderr: \n</code_execution_result>";
        assert!(!code_execution_has_evidence(content));
    }

    #[test]
    fn empty_dense_fallback_is_not_evidence() {
        let mode = rag_mode();
        let results = vec![ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": []})),
            trace: None,
        }];
        assert!(!has_retrieval_observation(&[], &results, &mode));
    }

    #[test]
    fn dense_fallback_with_chunks_counts_as_evidence() {
        let mode = rag_mode();
        let results = vec![ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": [{"chunk_id": "c1"}]})),
            trace: None,
        }];
        assert!(has_retrieval_observation(&[], &results, &mode));
    }

    #[test]
    fn blocks_content_early_stop_when_no_evidence() {
        let loop_exit = LoopExitConfig {
            require_evidence: true,
            allow_content_early_stop: false,
            skip_synthesis_on_direct_answer: false,
        };
        assert!(should_block_content_early_stop(&loop_exit, false));
        assert!(!should_block_content_early_stop(&loop_exit, true));
    }

    #[test]
    fn degraded_when_require_evidence_and_none() {
        let loop_exit = LoopExitConfig::default();
        assert_eq!(
            decide_post_loop(&loop_exit, false),
            PostLoopAction::DegradedNoEvidence
        );
        assert_eq!(
            decide_post_loop(&loop_exit, true),
            PostLoopAction::EnterSynthesis
        );
    }

    fn search_mode() -> ModeConfig {
        super::super::config::load_mode_config("search").unwrap()
    }

    fn empty_web_search() -> ToolResult {
        ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({"results": []})),
            trace: None,
        }
    }

    fn hit_web_search() -> ToolResult {
        ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "results": [{"url": "https://a.example", "title": "A", "snippet": "x"}]
            })),
            trace: None,
        }
    }

    #[test]
    fn empty_web_search_ok_is_not_evidence() {
        let mode = search_mode();
        assert!(!has_retrieval_observation(&[], &[empty_web_search()], &mode));
        assert!(!tool_result_has_web_hits(&empty_web_search()));
    }

    #[test]
    fn web_search_with_results_is_evidence() {
        let mode = search_mode();
        assert!(has_retrieval_observation(&[], &[hit_web_search()], &mode));
        assert!(tool_result_has_web_hits(&hit_web_search()));
    }

    #[test]
    fn consecutive_empty_search_triggers_early_stop() {
        let mode = search_mode();
        let two = vec![empty_web_search(), empty_web_search()];
        assert_eq!(consecutive_empty_search_tail(&two), 2);
        assert!(should_early_stop_search_on_empty(&mode, &two));
        // A hit resets the trail.
        let mixed = vec![empty_web_search(), hit_web_search(), empty_web_search()];
        assert_eq!(consecutive_empty_search_tail(&mixed), 1);
        assert!(!should_early_stop_search_on_empty(&mode, &mixed));
    }
}
