//! P1-4 / P1-5: Format output — ppt-generation and html-renderer.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext, assertions::*};

#[tokio::test]
async fn chat_presentation_html_returns_structured_slides() {
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let http_resp = ctx
        .chat_with_format_hint_without_mock_chunk_pin(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id],
            Some("ppt-generation"),
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);

    let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();

    let answer_lower = resp.answer.to_lowercase();
    assert!(
        answer_lower.contains("slide"),
        "expected 'slide' in formatted answer, got: {}",
        resp.answer
    );
    assert!(
        answer_lower.contains("<html") || answer_lower.contains("<!doctype"),
        "expected HTML structure in formatted answer, got: {}",
        resp.answer
    );
}

#[tokio::test]
async fn chat_html_renderer_returns_valid_html() {
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let http_resp = ctx
        .chat_with_format_hint_without_mock_chunk_pin(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id],
            Some("html-renderer"),
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);

    let resp: ChatResponse = serde_json::from_value(http_resp.body_json).unwrap();

    let answer_lower = resp.answer.to_lowercase();
    assert!(
        answer_lower.contains("<html"),
        "expected '<html' in formatted answer, got: {}",
        resp.answer
    );
    assert!(
        answer_lower.contains("<body"),
        "expected '<body' in formatted answer, got: {}",
        resp.answer
    );
}
