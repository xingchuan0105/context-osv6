//! P2-8: Corrupted / non-PDF file uploaded as PDF returns failed status.

use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

#[tokio::test]
#[ignore = "requires a genuinely corrupted fixture or server-side parser failure mode"]
async fn corrupted_file_upload_returns_failed_status() {
    let ctx = TestContext::new_smoke().await;

    // 1. Upload a file that claims to be a PDF but is actually random bytes.
    // For now we use empty.txt renamed to trigger parser confusion.
    let notebook = ctx.create_notebook("test-notebook").await.unwrap();
    let content = b"%PDF-1.4\n1 0 obj\n<<\n/Type /Catalog\n>>\nendobj\ntrailer\n<<\n/Root 1 0 R\n>>\n%%EOF\nnot_a_real_pdf";

    let resp = ctx
        .http_client
        .post(format!(
            "{}/api/v1/notebooks/{}/documents",
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

    // PUT the fake bytes
    let upload_resp = ctx
        .http_client
        .put(format!("{}/dev-upload/{doc_id}", ctx.base_url))
        .body(content.to_vec())
        .send()
        .await
        .unwrap();
    assert!(upload_resp.status().is_success());

    // 2. Wait for ingestion — should eventually fail
    let status = ctx
        .wait_for_ingestion(&doc_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(
        status,
        DocumentStatus::Failed,
        "corrupted PDF should fail ingestion, got: {:?}",
        status
    );
}
