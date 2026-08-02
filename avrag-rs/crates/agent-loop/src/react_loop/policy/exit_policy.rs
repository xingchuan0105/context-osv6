use avrag_llm::ChatMessage;
use contracts::{ToolResult, ToolStatus};

use super::config::{LoopExitConfig, ModeConfig};
use super::super::answer_contract::{
    contains_host_observation_shell, is_code_only_answer,
};

/// Tools whose payloads count as **answer-grade chunks** (unlock final answer).
/// Catalog-only tools (`doc_profile` / `doc_metadata`) are intentionally excluded:
/// a workspace listing is not retrieval evidence and must not open the answer gate.
/// Internal tool ids on bridge-captured `ToolResult`s (not LLM-facing schemas).
const RAG_ANSWER_CHUNK_TOOLS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "graph_retrieval",
    "index_lookup",
    "doc_summary",
    "doc_grep",
    "doc_read_lines",
    "struct_query",
];

/// Web evidence from SaC host (still tagged `web_search` / `web_fetch` on capture).
const SEARCH_EVIDENCE_TOOLS: &[&str] = &["web_search", "web_fetch"];

// LLM-facing observation bodies live in `prompts/loop/*.md` (see `prompt_assets`).
// Re-export loaders so call sites stay under exit_policy without inlined prose.
//
// No-chunk / host-forced **continue budget** is **+50% baseline max_tokens**
// (see `BudgetConfig::resolve_continue_token_boost`), not “+N free rounds”.
pub use super::super::prompt_assets::{
    no_chunk_budget_grace_nudge, no_chunk_continue_nudge, retrieval_failed_final_turn,
};

// ---------------------------------------------------------------------------
// Synthesis gate
// ---------------------------------------------------------------------------

/// Post-retrieve synthesis routing. Host does **not** force auto_fallback or
/// DegradedNoEvidence for missing chunks (skill-owned stop; retired 2026-07-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynthesisGate {
    EnterSynthesis,
    SkipSynthesisUseDirect(String),
}

pub fn decide_synthesis_gate(
    loop_exit: &LoopExitConfig,
    has_evidence: bool,
    direct_answer: Option<&str>,
    _tool_results: &[ToolResult],
    _query: &str,
) -> SynthesisGate {
    // require_evidence is skill-owned (no host hard gate). Direct answer /
    // synthesis routing does not refuse stop for missing chunks.
    let _ = has_evidence;
    let _ = loop_exit.require_evidence;

    if let Some(answer) = direct_answer {
        if loop_exit.skip_synthesis_on_direct_answer {
            // A DirectAnswer that is code-only or pastes a host observation
            // shell must not surface as final prose — same class as the
            // synthesis prose-only contract. Route to synthesis (which runs
            // the one-repair-round gate) instead of short-circuiting.
            if !is_code_only_answer(answer) && !contains_host_observation_shell(answer) {
                return SynthesisGate::SkipSynthesisUseDirect(answer.to_string());
            }
        }
    }

    SynthesisGate::EnterSynthesis
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
        // C8c: non-placeholder stdout alone is NOT evidence — it must carry a
        // chunk carrier (uuid-shaped chunk/doc id), otherwise fabricated
        // prose ("no_investor_info_found") would suppress the
        // DegradedNoEvidence refusal path. Bridge-captured tool results stay
        // an independent evidence signal via `tool_result_has_chunks`.
        if !stdout_is_placeholder(stdout) && stdout_has_chunk_carrier(stdout) {
            return true;
        }
    }
    false
}

/// Marker of a real retrieval hit in sandbox stdout: a canonical uuid-shaped
/// chunk/doc id (8-4-4-4-12 hex). The retrieval bridge prints one leading id
/// pair per hit line (`<chunk_id> <doc_id> <text>`), and the empty-stdout
/// fallback serializes captured chunks as JSON with `"chunk_id"` values — so
/// real hits always carry the marker while prose answers never do.
fn stdout_has_chunk_carrier(stdout: &str) -> bool {
    use std::sync::OnceLock;
    static UUID_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = UUID_RE.get_or_init(|| {
        regex::Regex::new(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        )
        .expect("uuid regex compiles")
    });
    re.is_match(stdout)
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

/// True when a RAG tool result carries at least one answer-grade chunk/item.
pub fn tool_result_has_chunks(result: &ToolResult) -> bool {
    if result.status != ToolStatus::Ok {
        return false;
    }
    if !RAG_ANSWER_CHUNK_TOOLS.contains(&result.tool.as_str()) {
        return false;
    }
    let Some(data) = result.data.as_ref() else {
        return false;
    };
    match result.tool.as_str() {
        "doc_grep" => {
            // total_hits > 0 is enough even when returned hits are truncated.
            let hits = data.get("total_hits").and_then(|v| v.as_u64()).unwrap_or(0) > 0;
            hits || chunk_array_non_empty(data)
                || data
                    .get("hits")
                    .and_then(|h| h.as_array())
                    .is_some_and(|a| !a.is_empty())
        }
        "doc_read_lines" => {
            data.get("lines")
                .and_then(|l| l.as_array())
                .is_some_and(|a| !a.is_empty())
                || chunk_array_non_empty(data)
        }
        "struct_query" => {
            // Table-query evidence: non-empty `chunks` (table-level md) or `rows`
            // (result set) counts as answer-grade material even before citation.
            chunk_array_non_empty(data)
                || data
                    .get("rows")
                    .and_then(|r| r.as_array())
                    .is_some_and(|a| !a.is_empty())
        }
        _ => chunk_array_non_empty(data),
    }
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

/// Formerly host hard gate for require_evidence. Always false: stop/grounding
/// is model+skill owned (AGENTS.md). Kept for call-site compatibility/tests.
pub fn should_block_content_early_stop(_loop_exit: &LoopExitConfig, _has_evidence: bool) -> bool {
    false
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
            "<code_execution_result>\n[block 0] stdout: 6c16ac99-e934-4355-be1c-f0956acb51d1 5a6de5e8-e913-46c9-a109-43eb65ae4e79 chunk text\nstderr: \n</code_execution_result>",
        )];
        assert!(has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn detects_code_execution_observation_with_untrusted_attribute() {
        // The opening tag may carry attributes (e.g. untrusted="true"). Parsing must still
        // match on the tag prefix, and the closing tag must remain recognized.
        let content = "<code_execution_result untrusted=\"true\">\n[block 0] stdout: 6c16ac99-e934-4355-be1c-f0956acb51d1 5a6de5e8-e913-46c9-a109-43eb65ae4e79 chunk text\nstderr: \n</code_execution_result>";
        assert!(has_code_execution_result_block(content));
        assert!(code_execution_has_evidence(content));
        let mode = rag_mode();
        let messages = vec![ChatMessage::user(content)];
        assert!(has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn prose_stdout_without_chunk_carrier_is_not_evidence() {
        // C8c: fabricated prose stdout (the "no_investor_info_found" class) has
        // no uuid chunk carrier → NOT evidence, so the DegradedNoEvidence
        // refusal path is reachable again.
        let content = "<code_execution_result>\n[block 0] stdout: no_investor_info_found\nstderr: \n</code_execution_result>";
        assert!(!code_execution_has_evidence(content));
        let mode = rag_mode();
        let messages = vec![ChatMessage::user(content)];
        assert!(!has_retrieval_observation(&messages, &[], &mode));
    }

    #[test]
    fn stdout_with_real_chunk_line_is_evidence() {
        // C8c: the bridge's real hit line shape `<chunk_id> <doc_id> <text>`.
        let content = "<code_execution_result>\n[block 0] stdout: === dense_search results ===\n6c16ac99-e934-4355-be1c-f0956acb51d1 5a6de5e8-e913-46c9-a109-43eb65ae4e79 从这个角度讲\nstderr: \n</code_execution_result>";
        assert!(code_execution_has_evidence(content));
        // …and the empty-stdout bridge fallback (JSON with chunk_id values).
        let fallback = "<code_execution_result>\n[block 0] stdout: [{\"chunk_id\":\"6c16ac99-e934-4355-be1c-f0956acb51d1\",\"text\":\"alpha\"}]\nstderr: \n</code_execution_result>";
        assert!(code_execution_has_evidence(fallback));
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
    fn host_never_blocks_content_for_require_evidence() {
        // require_evidence is skill-owned; host hard gate retired.
        let loop_exit = LoopExitConfig {
            require_evidence: true,
            allow_content_early_stop: false,
            skip_synthesis_on_direct_answer: false,
        };
        assert!(!should_block_content_early_stop(&loop_exit, false));
        assert!(!should_block_content_early_stop(&loop_exit, true));
        let loop_exit2 = LoopExitConfig {
            require_evidence: true,
            allow_content_early_stop: true,
            skip_synthesis_on_direct_answer: true,
        };
        assert!(!should_block_content_early_stop(&loop_exit2, false));
    }

    #[test]
    fn doc_profile_alone_is_not_answer_evidence() {
        let mode = rag_mode();
        let results = vec![ToolResult {
            tool: "doc_profile".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!([{"name": "thesis.txt", "doc_id": "x"}])),
            trace: None,
        }];
        assert!(!has_retrieval_observation(&[], &results, &mode));
    }

    #[test]
    fn doc_grep_with_hits_counts_as_answer_evidence() {
        let mode = rag_mode();
        let results = vec![ToolResult {
            tool: "doc_grep".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "total_hits": 3,
                "returned": 3,
                "truncated": false,
                "hits": [{"line": 1, "text": "x"}],
                "chunks": [{"chunk_id": "6c16ac99-e934-4355-be1c-f0956acb51d1"}]
            })),
            trace: None,
        }];
        assert!(has_retrieval_observation(&[], &results, &mode));
    }

    #[test]
    fn decide_synthesis_gate_allows_direct_without_chunks() {
        // Host no longer refuses direct answer for missing evidence.
        let loop_exit = LoopExitConfig {
            require_evidence: true,
            allow_content_early_stop: true,
            skip_synthesis_on_direct_answer: true,
        };
        assert_eq!(
            decide_synthesis_gate(&loop_exit, false, Some("model prose"), &[], "q"),
            SynthesisGate::SkipSynthesisUseDirect("model prose".to_string())
        );
    }

    #[test]
    fn decide_synthesis_gate_routes_code_only_direct_to_synthesis() {
        let loop_exit = LoopExitConfig {
            require_evidence: true,
            allow_content_early_stop: true,
            skip_synthesis_on_direct_answer: true,
        };
        // Code-only and host-observation-shell direct answers must not
        // short-circuit; they route to synthesis (which runs the prose-only
        // repair gate). Prose is unaffected.
        assert_eq!(
            decide_synthesis_gate(
                &loop_exit,
                false,
                Some("<code language=\"python\">\nprint(1)\n</code>"),
                &[],
                "q"
            ),
            SynthesisGate::EnterSynthesis
        );
        assert_eq!(
            decide_synthesis_gate(
                &loop_exit,
                false,
                Some("<retrieval_summary>\nfake\n</retrieval_summary>"),
                &[],
                "q"
            ),
            SynthesisGate::EnterSynthesis
        );
        assert_eq!(
            decide_synthesis_gate(&loop_exit, false, Some("```python\nprint(1)\n```"), &[], "q"),
            SynthesisGate::EnterSynthesis
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
        assert!(!has_retrieval_observation(
            &[],
            &[empty_web_search()],
            &mode
        ));
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
