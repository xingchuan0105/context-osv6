//! Real-LLM format output E2E regression tests.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::format_real -- --ignored --test-threads=1 --nocapture

use crate::product_e2e::{
    assertions::assert_answer_substantive,
    fixtures::shared_standard_doc_real_llm,
    llm_real::{chat_with_format_retry, merge_llm_real_extra, non_blocking_degrade},
};

/// Real LLM + html-renderer format skill returns HTTP 200 with HTML content.
/// Reuses cold-ingested standard doc ([`shared_standard_doc_real_llm`]).
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_format_html_renderer_returns_html() {
    super::require_nightly_suite();
    let (ctx, upload) = shared_standard_doc_real_llm().await;

    let result = chat_with_format_retry(
        &ctx,
        "Render the document content as an HTML page",
        &upload.workspace_id,
        &[upload.document_id.clone()],
        "html-renderer",
    )
    .await;
    let resp = &result.resp;

    let answer_lower = resp.answer.to_lowercase();
    assert!(
        answer_lower.contains("<html"),
        "expected '<html' in formatted answer, got: {}",
        resp.answer.chars().take(200).collect::<String>()
    );
    assert!(
        answer_lower.contains("<body"),
        "expected '<body' in formatted answer, got: {}",
        resp.answer.chars().take(200).collect::<String>()
    );
    assert_answer_substantive(resp, 30);
    let blocking_degrades: Vec<_> = resp
        .degrade_trace
        .iter()
        .filter(|item| !non_blocking_degrade(item))
        .collect();
    assert!(
        blocking_degrades.is_empty(),
        "expected no blocking degradation on format happy path, got: {:?}",
        blocking_degrades
    );

    ctx.save_llm_artifact(
        "real_llm_format_html_renderer_returns_html",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "document_id": upload.document_id,
                "format_hint": "html-renderer",
            })),
        ),
        Some(result.reasoning),
    );
}
