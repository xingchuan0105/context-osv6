//! Assertion helpers for Product E2E.
//!
//! Three layers (per plan §4):
//! - Protocol: HTTP status, schema validity, field presence
//! - Product: business rules (citation types, doc_id matching, degrade traces)
//! - Quality: NOT here — only for nightly / offline evaluation

use crate::product_e2e::{ChatResponse, HttpResponse};

// ---------------------------------------------------------------------------
// Protocol layer assertions
// ---------------------------------------------------------------------------

/// Assert HTTP 200 OK.
pub fn assert_http_ok(resp: &HttpResponse) {
    assert_eq!(
        resp.status, 200,
        "expected HTTP 200, got {}. body: {}",
        resp.status,
        resp.body_json
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

/// Assert at least one citation has source_type == "document".
pub fn assert_answer_has_doc_citation(resp: &ChatResponse) {
    // Note: production Citation does not have `source_type` field today.
    // This assertion will need a field addition or inference heuristic.
    // For now, assert that doc_id is non-empty (all doc citations have doc_id).
    let has_doc = resp
        .citations
        .iter()
        .any(|c| !c.doc_id.is_empty() && c.doc_id != "web");
    assert!(
        has_doc,
        "expected at least one document citation, got: {:?}",
        resp.citations
    );
}

/// Assert at least one citation has doc_id == "web" (web search citation).
///
/// TODO: production Citation schema may need a `source_type` field.
/// Until then, this assertion uses a convention: web citations use doc_id "web".
pub fn assert_answer_has_web_citation(resp: &ChatResponse) {
    let has_web = resp.citations.iter().any(|c| c.doc_id == "web");
    assert!(
        has_web,
        "expected at least one web citation, got doc_ids: {:?}",
        resp.citations.iter().map(|c| &c.doc_id).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Product layer assertions — degrade trace
// ---------------------------------------------------------------------------

/// Assert degrade_trace contains at least one entry with the expected reason.
pub fn assert_degrade_reason(resp: &ChatResponse, expected_reason: &str) {
    let reasons: Vec<&str> = resp
        .degrade_trace
        .iter()
        .map(|d| d.reason.as_str())
        .collect();
    assert!(
        reasons.iter().any(|r| *r == expected_reason),
        "expected degrade_trace reason '{}', got: {:?}",
        expected_reason,
        reasons
    );
}

/// Assert degrade_trace is non-empty (any fallback occurred).
pub fn assert_has_degrade_trace(resp: &ChatResponse) {
    assert!(
        !resp.degrade_trace.is_empty(),
        "expected non-empty degrade_trace, but got none"
    );
}

// ---------------------------------------------------------------------------
// Product layer assertions — format output
// ---------------------------------------------------------------------------

/// Assert response contains a format_output block with the expected type.
///
/// TODO: ChatResponse does not currently have `format_output` field.
/// This will be enabled when the field is added to the production schema.
pub fn assert_format_output_type(resp: &ChatResponse, expected_type: &str) {
    // Placeholder — will scan answer_blocks or a future format_output field.
    let _ = expected_type;
    assert!(
        !resp.answer_blocks.is_empty() || !resp.answer.is_empty(),
        "expected formatted output, got empty answer. response: {:?}",
        resp
    );
}

// ---------------------------------------------------------------------------
// Product layer assertions — answer substance
// ---------------------------------------------------------------------------

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
