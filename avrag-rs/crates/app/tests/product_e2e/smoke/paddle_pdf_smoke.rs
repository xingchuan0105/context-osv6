//! ING-2 acceptance: Black Swan p1–20 hybrid PDF routing (Paddle + ING-4).

use std::path::Path;
use std::time::Duration;

use crate::product_e2e::{DocumentStatus, TestContext};

fn black_swan_pdf_path() -> String {
    std::env::var("E2E_LLM_REAL_BLACK_SWAN_PDF").unwrap_or_else(|_| {
        "/mnt/e/OneDrive/桌面/知境笔记/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf".to_string()
    })
}

fn apply_paddle_smoke_profile() {
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", "20");
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

#[tokio::test]
#[ignore = "requires Black Swan PDF, office-parser, pdf-renderer, PaddleOCR; run with --ignored --test-threads=1"]
async fn black_swan_paddle_pdf_smoke() {
    apply_paddle_smoke_profile();
    let path = black_swan_pdf_path();
    assert!(
        Path::new(&path).is_file(),
        "PDF not found at {path}. Set E2E_LLM_REAL_BLACK_SWAN_PDF."
    );

    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_file_from_path(&path).await.expect("upload pdf");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(900))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);

    let text_chunks = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("text chunk count");
    assert!(text_chunks > 0, "expected text_chunks > 0, got {text_chunks}");

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    let summary_text = summary.to_string();
    assert!(
        summary_text.contains("slow_ocr")
            || summary_text.contains("\"C\"")
            || summary_text.contains("paddle")
            || summary_text.contains("PaddleOcr"),
        "expected paddle/C routing in backend_summary: {summary_text}"
    );
    assert!(
        summary.get("page_status").is_some(),
        "page_status should be present in backend_summary"
    );

    let page_raster_mm = ctx
        .query_multimodal_page_raster_count(&upload.document_id)
        .await
        .expect("page_raster multimodal count");
    assert_eq!(
        page_raster_mm,
        0,
        "ING-4: page_raster multimodal chunks should be 0 when Paddle OCR succeeds"
    );
}
