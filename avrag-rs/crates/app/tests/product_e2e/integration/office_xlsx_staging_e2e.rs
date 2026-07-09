//! Real Office JVM xlsx ingest (staging only; mock path covered by `office_xlsx_e2e`).

use std::time::Duration;

use crate::product_e2e::setup;
use crate::product_e2e::{DocumentStatus, TestContext};

fn contract_xlsx_path() -> std::path::PathBuf {
    setup::fixture_path("contract-xlsx.xlsx").expect("contract-xlsx.xlsx fixture")
}

async fn require_office_parser_health() {
    let base = std::env::var("OFFICE_PARSER_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());
    let health = format!("{}/v1/healthz", base.trim_end_matches('/'));
    let ok = reqwest::Client::new()
        .get(&health)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    assert!(
        ok,
        "office-parser-jvm must be running at {health}. Start: ./scripts/office-parser-up.sh"
    );
}

#[tokio::test]
#[ignore = "requires real office-parser-jvm; staging only"]
async fn office_xlsx_staging_ingest_e2e() {
    super::require_integration_suite();
    require_office_parser_health().await;
    assert!(
        contract_xlsx_path().is_file(),
        "missing bundled fixture contract-xlsx.xlsx"
    );

    let mut ctx = TestContext::new_with_real_llm().await;
    let path = contract_xlsx_path().to_string_lossy().to_string();
    let upload = ctx.upload_file_from_path(&path).await.expect("upload xlsx");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(240))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    let summary_text = summary.to_string();
    assert!(
        summary_text.contains("office") || summary_text.contains("xlsx"),
        "expected office/xlsx routing in backend_summary: {summary_text}"
    );

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("chunk count");
    assert!(
        chunk_count > 0,
        "expected indexed chunks after real xlsx ingest"
    );
}
