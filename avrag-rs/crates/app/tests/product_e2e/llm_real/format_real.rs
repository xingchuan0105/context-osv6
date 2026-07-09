//! Real-LLM format output E2E regression tests.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::format_real -- --ignored --test-threads=1 --nocapture

use std::time::Duration;

use crate::product_e2e::{
    DegradeReason, DocumentStatus, TestContext,
    assertions::assert_answer_substantive,
    llm_real::{chat_with_format_retry, merge_llm_real_extra},
};

/// Real LLM + html-renderer format skill returns HTTP 200 with HTML content.
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_format_html_renderer_returns_html() {
    super::require_nightly_suite();
    let mut ctx = TestContext::new_with_real_llm().await;

    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload document");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("ingest document");
    assert_eq!(status, DocumentStatus::Completed);

    let result = chat_with_format_retry(
        &ctx,
        "Render the document content as an HTML page",
        &upload.notebook_id,
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
        .filter(|item| {
            !(item.stage == "dense_retrieval"
                && matches!(
                    &item.reason,
                    DegradeReason::Other(msg) if msg.contains("multimodal embedding input is empty")
                ))
        })
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
