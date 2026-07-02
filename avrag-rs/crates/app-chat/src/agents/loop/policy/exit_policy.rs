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
    DegradedNoEvidence,
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

/// Returns true when a `<code_execution_result>` observation carries retrieval output.
pub fn code_execution_has_evidence(message_content: &str) -> bool {
    let Some(inner) = message_content
        .split("<code_execution_result>")
        .nth(1)
        .and_then(|s| s.split("</code_execution_result>").next())
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

pub fn has_retrieval_observation(
    messages: &[ChatMessage],
    collected_tool_results: &[ToolResult],
    mode: &ModeConfig,
) -> bool {
    if mode.id == "rag" {
        if messages.iter().any(|m| {
            m.role == "user"
                && m.content.contains("<code_execution_result>")
                && code_execution_has_evidence(&m.content)
        }) {
            return true;
        }
        return collected_tool_results.iter().any(tool_result_has_chunks);
    }
    if mode.id == "search" {
        if collected_tool_results
            .iter()
            .any(|r| r.status == ToolStatus::Ok && SEARCH_EVIDENCE_TOOLS.contains(&r.tool.as_str()))
        {
            return true;
        }
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
}
