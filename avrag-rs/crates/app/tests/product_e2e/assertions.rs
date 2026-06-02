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
    let has_web = resp.citations.iter().any(|c| c.layer.as_deref() == Some("search"));
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
