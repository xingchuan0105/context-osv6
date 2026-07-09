//! P2-8: Corrupted / non-PDF file uploaded as PDF returns failed status.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
async fn corrupted_file_upload_returns_failed_status() {
    super::require_integration_suite();

    let mut ctx = TestContext::new_smoke().await;

    // 1. Upload a file named .pdf with MIME application/pdf, but the body
    //    is actually a PNG image.  ParseRouter accepts extension↔MIME, but
    //    lopdf::Document::load_mem will fail when it tries to parse the PNG
    //    bytes as a PDF, causing ingestion to transition to Failed.
    let notebook = ctx.create_workspace("test-notebook").await.unwrap();
    // Minimal valid PNG (1×1 pixel, transparent)
    let content = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\xfc\xcf\xc0\x50\x0f\x00\x04A\x01\xa1\x3a\xf0\xfc\xcc\x00\x00\x00\x00IEND\xaeB`\x82";

    let resp = ctx
        .http_client
        .post(format!(
            "{}/api/v1/workspaces/{}/documents",
            ctx.base_url, notebook.id
        ))
        .json(&serde_json::json!({
            "filename": "corrupted.pdf",
            "file_size": content.len(),
            "mime_type": "application/pdf",
        }))
        .send()
        .await
        .unwrap();
    let body = resp.json::<serde_json::Value>().await.unwrap();
    let doc_id = body["document_id"].as_str().unwrap().to_string();

    let upload_resp = ctx
        .http_client
        .put(format!("{}/dev-upload/{doc_id}", ctx.base_url))
        .body(content.to_vec())
        .send()
        .await
        .unwrap();
    assert!(upload_resp.status().is_success());

    // 2. Force max_attempts = 1 so the first parser failure dead-letters immediately
    //    instead of retrying through the default 5-attempt backoff chain (≈ 7.5 min).
    ctx.set_ingestion_max_attempts(&doc_id, 1)
        .await
        .expect("set max_attempts");

    // 3. Wait for ingestion — lopdf should fail to parse the PNG-as-PDF
    let status = ctx
        .wait_for_ingestion(&doc_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(
        status,
        DocumentStatus::Failed,
        "corrupted PDF (PNG body) should fail ingestion, got: {:?}",
        status
    );
}
