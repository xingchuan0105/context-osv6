use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use contracts::chat::{ChatRequest, ChatResponse, Citation, DegradeTraceItem, ModeDebug, PlannerOutput, RagModeDebug, RagPlan, RagTraceItem, RagTraceSummary, SummaryInjectionTrace, TraceInfo};
use uuid::Uuid;

use super::planner::rag_summary_mode;
use super::{FINAL_MIN_CHUNKS, FINAL_RERANK_BUDGET, TOTAL_CANDIDATE_BUDGET};

pub(super) fn materialize_answer_markup(answer_text: &str, citations: &[Citation]) -> String {
    let citation_index_by_chunk = citations
        .iter()
        .filter_map(|citation| {
            citation
                .chunk_id
                .as_ref()
                .map(|chunk_id| (chunk_id.clone(), citation.citation_id))
        })
        .collect::<HashMap<_, _>>();
    let mut rendered = String::new();
    let mut remaining = answer_text;
    let mut replaced_any = false;

    while let Some(start) = remaining.find("[[") {
        rendered.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            rendered.push_str(&remaining[start..]);
            remaining = "";
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if let Some(citation_id) = citation_index_by_chunk.get(chunk_id) {
                write!(rendered, "[[{citation_id}]]").unwrap();
                replaced_any = true;
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim) {
            if let Some(citation_id) = citation_index_by_chunk.get(chunk_id) {
                write!(rendered, "[[image:{citation_id}]]").unwrap();
                replaced_any = true;
            }
        } else {
            rendered.push_str(&remaining[start..start + 2 + end + 2]);
        }
        remaining = &after_start[end + 2..];
    }
    rendered.push_str(remaining);

    if replaced_any || citations.is_empty() {
        return rendered;
    }

    let inline_refs = citations
        .iter()
        .take(2)
        .map(|citation| format!("[[{}]]", citation.citation_id))
        .collect::<Vec<_>>()
        .join(" ");
    if inline_refs.is_empty() {
        rendered
    } else {
        format!("{} {}", rendered.trim_end(), inline_refs)
    }
}

pub(super) fn extract_referenced_chunk_ids(answer_text: &str) -> HashSet<String> {
    let mut remaining = answer_text;
    let mut ids = HashSet::new();
    while let Some(start) = remaining.find("[[") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if !chunk_id.is_empty() {
                ids.insert(chunk_id.to_string());
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim)
            && !chunk_id.is_empty()
        {
            ids.insert(chunk_id.to_string());
        }
        remaining = &after_start[end + 2..];
    }
    ids
}

pub(super) fn ensure_inline_image_placeholder(answer_text: &str, citations: &[Citation]) -> String {
    if answer_text.contains("[[image:") {
        return answer_text.to_string();
    }

    let Some(image_citation) = citations.iter().find(|citation| {
        citation
            .image_url
            .as_ref()
            .is_some_and(|url| !url.trim().is_empty())
    }) else {
        return answer_text.to_string();
    };

    let Some(chunk_id) = image_citation.chunk_id.as_deref() else {
        return answer_text.to_string();
    };

    format!("{}\n\n[[image:{}]]", answer_text.trim_end(), chunk_id)
}

pub(super) fn no_chunks_response(
    request: &ChatRequest,
    rag_plan: &RagPlan,
    item_trace: &[RagTraceItem],
    degrade_trace: Vec<DegradeTraceItem>,
    answer: String,
) -> ChatResponse {
    let summary_mode = rag_summary_mode(rag_plan);
    let answer_blocks = common::plain_text_answer_blocks(&answer);

    ChatResponse {
        answer,
        answer_blocks,
        session_id: request
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        agent_type: request.agent_type.clone(),
        sources: Vec::new(),
        citations: Vec::new(),
        trace: TraceInfo {
            mode: "rag".to_string(),
        },
        degrade_trace,
        planner_output: Some(PlannerOutput {
            mode: "rag".to_string(),
            rag_plan: Some(rag_plan.clone()),
            search_plan: None,
            general_plan: None,
        }),
        mode_debug: Some(ModeDebug {
            rag: Some(RagModeDebug {
                item_trace: item_trace.to_vec(),
                retrieval_trace: RagTraceSummary {
                    item_count: item_trace.len(),
                    total_candidate_budget: TOTAL_CANDIDATE_BUDGET,
                    max_rerank_docs: FINAL_RERANK_BUDGET,
                    max_final_chunks: FINAL_MIN_CHUNKS,
                    top_k_returned: 0,
                    summary_mode: summary_mode.clone(),
                    items: item_trace.to_vec(),
                },
                summary_injection_trace: SummaryInjectionTrace {
                    mode: summary_mode,
                    injected_count: 0,
                },
            }),
            search: None,
            general: None,
        }),
        message_id: None,
        guard_report: None,
        tool_results: Vec::new(),
        usage: None,
    }
}
