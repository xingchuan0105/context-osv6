//! Assertion helpers for Product E2E.
//!
//! Three layers (per plan §4):
//! - Protocol: HTTP status, schema validity, field presence
//! - Product: business rules (citation types, doc_id matching, degrade traces)
//! - Quality: NOT here — only for nightly / offline evaluation

use crate::product_e2e::{ChatResponse, DegradeReason, HttpResponse};

// ---------------------------------------------------------------------------
// Protocol layer assertions
// ---------------------------------------------------------------------------

/// Assert HTTP 200 OK.
pub fn assert_http_ok(resp: &HttpResponse) {
    assert_eq!(
        resp.status, 200,
        "expected HTTP 200, got {}. body: {}",
        resp.status, resp.body_json
    );
}

/// Assert HTTP status code matches expected value.
pub fn assert_http_status(resp: &HttpResponse, expected: u16) {
    assert_eq!(
        resp.status, expected,
        "expected HTTP {}, got {}. body: {}",
        expected, resp.status, resp.body_json
    );
}

/// Assert response body JSON contains the given key.
pub fn assert_has_json_key(resp: &HttpResponse, key: &str) {
    assert!(
        resp.body_json.get(key).is_some(),
        "expected JSON key '{}' in body: {}",
        key,
        resp.body_json
    );
}

// ---------------------------------------------------------------------------
// Product layer assertions — citations
// ---------------------------------------------------------------------------

/// Assert ChatResponse has at least one citation.
pub fn assert_has_citations(resp: &ChatResponse) {
    assert!(
        !resp.citations.is_empty(),
        "expected at least one citation, got none. answer: {}",
        resp.answer
    );
}

/// Assert at least one citation comes from the expected doc_id.
pub fn assert_citation_doc_id(resp: &ChatResponse, expected_doc_id: &str) {
    let ids: Vec<&str> = resp.citations.iter().map(|c| c.doc_id.as_str()).collect();
    assert!(
        ids.contains(&expected_doc_id),
        "expected citation from doc_id '{}', got doc_ids: {:?}",
        expected_doc_id,
        ids
    );
}

/// Assert at least one citation comes from a document (not web search).
///
/// Uses `Citation.layer`: web citations have `layer == Some("search")`,
/// document citations have any other value (typically `None`).
pub fn assert_answer_has_doc_citation(resp: &ChatResponse) {
    let has_doc = resp
        .citations
        .iter()
        .any(|c| c.layer.as_deref() != Some("search"));
    assert!(
        has_doc,
        "expected at least one document citation (layer != 'search'), got: {:?}",
        resp.citations
    );
}

/// Assert at least one citation comes from web search (layer == "search").
///
/// Search mode sets `Citation.layer` to `"search"` and `doc_id` to the source URL.
pub fn assert_answer_has_web_citation(resp: &ChatResponse) {
    let has_web = resp
        .citations
        .iter()
        .any(|c| c.layer.as_deref() == Some("search"));
    assert!(
        has_web,
        "expected at least one web citation (layer == 'search'), got citations: {:?}",
        resp.citations
    );
}

// ---------------------------------------------------------------------------
// Product layer assertions — degrade trace
// ---------------------------------------------------------------------------
// (Previously had `assert_degrade_reason` and `assert_has_degrade_trace` helpers
// here; removed because they had no callers. Use `!resp.degrade_trace.is_empty()`
// inline or assert specific reasons via a dedicated helper once a use case arises.)
//
// ---------------------------------------------------------------------------
// Product layer assertions — format output
// ---------------------------------------------------------------------------
// (Previously had `assert_format_output_type` placeholder here; removed —
// `ChatResponse` has no `format_output` field yet. Re-introduce when the
// production schema gains one and write a real structural assertion.)
//

// ---------------------------------------------------------------------------
// Product layer assertions — answer substance
// ---------------------------------------------------------------------------

/// Assert observability contract: non-empty answer on HTTP 200, core fields present.
pub fn assert_observability_contract(resp: &ChatResponse) {
    assert!(
        !resp.answer.trim().is_empty(),
        "observability contract: answer must be non-empty, got {:?}",
        resp.answer
    );
    assert!(
        !resp.agent_type.trim().is_empty(),
        "observability contract: agent_type must be present"
    );
}

/// Assert degrade_trace contains an expected reason (stable enum match).
pub fn assert_degrade_reason(resp: &ChatResponse, expected: DegradeReason) {
    assert!(
        resp.degrade_trace.iter().any(|item| item.reason == expected),
        "expected degrade reason {expected:?}, got: {:?}",
        resp.degrade_trace
    );
}

/// Assert answer does NOT contain any of the forbidden keywords (case-insensitive).
pub fn assert_answer_excludes_keywords(resp: &ChatResponse, forbidden: &[&str]) {
    let answer_lower = resp.answer.to_lowercase();
    for kw in forbidden {
        let kw_lower = kw.to_lowercase();
        assert!(
            !answer_lower.contains(&kw_lower),
            "answer unexpectedly contains forbidden keyword '{}'. answer preview: {}",
            kw,
            resp.answer.chars().take(200).collect::<String>()
        );
    }
}

/// Assert answer has minimum length (not empty / trivial).
pub fn assert_answer_substantive(resp: &ChatResponse, min_len: usize) {
    assert!(
        resp.answer.len() >= min_len,
        "answer too short ({} chars, expected >= {}): {}",
        resp.answer.len(),
        min_len,
        resp.answer
    );
}

/// Assert answer explicitly references at least one citation chunk via `[[cite:CHUNK_ID]]`.
pub fn assert_citation_referenced_in_answer(resp: &ChatResponse) {
    if resp.citations.is_empty() {
        return;
    }
    let cited_chunk_ids = extract_answer_cited_chunk_ids(&resp.answer);
    assert!(
        resp.citations.iter().any(|citation| {
            citation
                .chunk_id
                .as_ref()
                .is_some_and(|id| cited_chunk_ids.contains(id))
        }),
        "expected answer to reference a citation chunk_id via [[cite:...]], answer={} citations={}",
        resp.answer.chars().take(120).collect::<String>(),
        resp.citations.len()
    );
}

fn extract_answer_cited_chunk_ids(answer: &str) -> std::collections::HashSet<String> {
    let mut remaining = answer;
    let mut ids = std::collections::HashSet::new();
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
        }
        remaining = &after_start[end + 2..];
    }
    ids
}

/// Assert codegen bridge captured a successful `dense_retrieval` tool result.
pub fn assert_codegen_bridge_dense_retrieval(resp: &ChatResponse) {
    let has_dense = resp.tool_results.iter().any(|result| {
        result.tool == "dense_retrieval" && result.status == contracts::chat::ToolStatus::Ok
    });
    assert!(
        has_dense,
        "expected dense_retrieval from codegen bridge in tool_results, got: {:?}",
        resp
            .tool_results
            .iter()
            .map(|r| (&r.tool, &r.status))
            .collect::<Vec<_>>()
    );
}

/// Assert cited chunk ids belong to the ingested document (proves bridge retrieval, not mock pin).
pub fn assert_citations_use_document_chunks(resp: &ChatResponse, document_chunk_ids: &[String]) {
    assert_has_citations(resp);
    let allowed: std::collections::HashSet<&str> =
        document_chunk_ids.iter().map(String::as_str).collect();
    for citation in &resp.citations {
        let Some(chunk_id) = citation.chunk_id.as_deref() else {
            continue;
        };
        assert!(
            allowed.contains(chunk_id),
            "citation chunk_id {chunk_id} must be from ingested document chunks {document_chunk_ids:?}"
        );
    }
}
