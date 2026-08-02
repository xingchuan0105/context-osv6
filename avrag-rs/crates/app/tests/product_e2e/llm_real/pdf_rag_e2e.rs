//! Real-LLM RAG after liteparse v2 PDF ingest (ingest + retrieval + synthesis).

use std::time::Duration;

use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{
        assert_answer_has_doc_citation, assert_answer_substantive, assert_citation_doc_id,
        assert_citation_referenced_in_answer, assert_has_citations,
        assert_no_mineru_in_backend_summary, assert_primary_backend,
    },
    llm_real::{
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS, chat_with_citations_retry_attempts, merge_llm_real_extra,
    },
    setup,
};

fn phase0_mini_fixture_path() -> std::path::PathBuf {
    setup::fixture_path("phase0-mini.pdf").expect("phase0-mini.pdf fixture")
}

fn apply_pdf_ingestion_profile() {
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", "8");
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

#[tokio::test]
#[ignore = "requires real LLM + embedding; run with --ignored --test-threads=1"]
async fn real_llm_rag_after_markitdown_pdf_ingest_returns_citation() {
    super::require_nightly_suite();
    apply_pdf_ingestion_profile();
    assert!(
        phase0_mini_fixture_path().is_file(),
        "missing bundled fixture phase0-mini.pdf"
    );

    let mut ctx = TestContext::new_with_real_llm().await;
    let notebook = ctx.create_workspace("pdf-rag-real").await.expect("notebook");
    let path = phase0_mini_fixture_path().to_string_lossy().to_string();
    let upload = ctx
        .upload_file_from_path_to_notebook(&path, &notebook.id)
        .await
        .expect("upload pdf");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(240))
        .await
        .expect("ingest pdf");
    assert_eq!(status, DocumentStatus::Completed);

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    assert_primary_backend(&summary, "liteparse_v2_pdf");
    assert_no_mineru_in_backend_summary(&summary);

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .expect("chunk count");
    assert!(
        chunk_count > 0,
        "expected chunks after liteparse PDF ingest"
    );

    let result = chat_with_citations_retry_attempts(
        &ctx,
        "According to the uploaded PDF, summarize its content and key findings. Cite the document.",
        &notebook.id,
        &[upload.document_id.clone()],
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS + 2,
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_has_doc_citation(resp);
    assert_citation_referenced_in_answer(resp);
    assert_answer_substantive(resp, 30);

    ctx.save_llm_artifact(
        "real_llm_rag_after_markitdown_pdf_ingest_returns_citation",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({"document_id": upload.document_id})),
        ),
        Some(result.reasoning),
    );
}
