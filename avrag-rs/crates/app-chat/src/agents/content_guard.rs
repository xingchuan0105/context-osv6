use avrag_guardrails::GuardPipeline;
use common::{ToolResult};
use contracts::chat::{DegradeReason, DegradeTraceItem};

const REDACTED_PLACEHOLDER: &str = "[REDACTED: content flagged by security guard]";

/// Sanitize RAG tool results by scanning chunk text for prompt injection.
/// Returns sanitized results + degrade trace items for any redactions.
pub fn sanitize_tool_results(
    tool_results: &[ToolResult],
    guard: &GuardPipeline,
    trace_id: Option<String>,
) -> (Vec<ToolResult>, Vec<DegradeTraceItem>) {
    let mut sanitized = tool_results.to_vec();
    let mut degrade_trace = Vec::new();

    for result in &mut sanitized {
        let Some(data) = result.data.as_mut().and_then(|d| d.as_array_mut()) else {
            continue;
        };
        for item in data {
            let Some(text_val) = item.get_mut("text") else {
                continue;
            };
            let Some(text) = text_val.as_str() else {
                continue;
            };

            let guard_result = match guard.check_content(text, trace_id.clone()) {
                Some(result) => result,
                None => continue,
            };
            if !guard_result.passed {
                *text_val = serde_json::json!(REDACTED_PLACEHOLDER);
                degrade_trace.push(DegradeTraceItem {
                    stage: "input_guard:content_sanitizer".into(),
                    reason: DegradeReason::ContentGuard,
                    impact: "redact".into(),
                });
            }
        }
    }

    (sanitized, degrade_trace)
}

use avrag_search::SearchResult;

/// Sanitize web search results by scanning snippets for prompt injection.
/// Returns sanitized results + degrade trace items for any redactions.
pub fn sanitize_search_results(
    results: &[SearchResult],
    guard: &GuardPipeline,
    trace_id: Option<String>,
) -> (Vec<SearchResult>, Vec<DegradeTraceItem>) {
    let mut sanitized = results.to_vec();
    let mut degrade_trace = Vec::new();

    for result in &mut sanitized {
        let guard_result = match guard.check_content(&result.snippet, trace_id.clone()) {
            Some(result) => result,
            None => continue,
        };
        if !guard_result.passed {
            result.snippet = REDACTED_PLACEHOLDER.to_string();
            degrade_trace.push(DegradeTraceItem {
                stage: "input_guard:content_sanitizer".into(),
                reason: DegradeReason::ContentGuard,
                impact: "redact".into(),
            });
        }
    }

    (sanitized, degrade_trace)
}
