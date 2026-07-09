//! Nightly PDF corpus — P4 LiteParse hybrid ingest + real-LLM RAG.
//!
//! Default: bundled `phase0-mini.pdf` (CI/staging portable).
//! Optional staging stress: set `E2E_LLM_REAL_STAGING_PDF` to a local book path.
//!
//! Run:
//!   E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e \
//!     llm_real::pdf_corpus -- --ignored --test-threads=1 --nocapture

use std::path::Path;
use std::time::Duration;

use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{
        assert_answer_substantive, assert_citation_doc_id, assert_has_citations,
        assert_liteparse_hybrid_backend_summary, assert_no_mineru_in_backend_summary,
    },
    llm_real::{
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS, chat_with_citations_retry,
        chat_with_citations_retry_attempts, chat_with_retry, merge_llm_real_extra,
    },
    setup,
};

const RETRIEVAL_TOOLS: &[&str] = &[
    "dense_retrieval",
    "index_lookup",
    "doc_profile",
    "doc_summary",
];
const BUNDLED_INGEST_TIMEOUT: Duration = Duration::from_secs(300);
const STAGING_INGEST_TIMEOUT: Duration = Duration::from_secs(1200);

/// P4 fast profile: 20 pages default, no VLM side-effects, no page-raster OCR shortcut.
fn apply_p4_pdf_ingestion_profile() {
    let max_pages = std::env::var("E2E_PDF_MAX_PAGES").unwrap_or_else(|_| "20".to_string());
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", &max_pages);
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
        std::env::set_var("INGESTION_PAGE_RASTER_WITH_OCR", "0");
    }
}

fn bundled_pdf_path() -> std::path::PathBuf {
    setup::fixture_path("phase0-mini.pdf").expect("phase0-mini.pdf fixture")
}

fn staging_pdf_path() -> Option<String> {
    std::env::var("E2E_LLM_REAL_STAGING_PDF")
        .ok()
        .filter(|p| Path::new(p).is_file())
}

async fn ingest_pdf_with_routing_asserts(
    ctx: &mut TestContext,
    path: &str,
    timeout: Duration,
    min_chunk_units: usize,
) -> crate::product_e2e::UploadResponse {
    eprintln!("[llm_real/pdf_corpus] uploading {path}");
    let upload = ctx.upload_file_from_path(path).await.expect("upload pdf");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, timeout)
        .await
        .expect("ingest pdf");
    if status != DocumentStatus::Completed {
        let detail = ctx
            .fetch_document_status(&upload.document_id)
            .await
            .unwrap_or_else(|e| serde_json::json!({ "fetch_error": e.to_string() }));
        let worker_tail = ctx.worker_log_tail(400);
        ctx.save_failure_artifacts(
            "real_llm_pdf_ingestion_failed",
            Some(&serde_json::json!({
                "pdf_path": path,
                "document_id": upload.document_id,
                "status": format!("{status:?}"),
                "detail": detail,
            })),
        );
        panic!(
            "PDF ingestion failed for {}: status={status:?}, detail={}. worker_log_tail:\n{worker_tail}",
            upload.document_id,
            serde_json::to_string_pretty(&detail).unwrap_or_default()
        );
    }

    let summary = ctx
        .query_latest_backend_summary(&upload.document_id)
        .await
        .expect("backend_summary");
    assert_liteparse_hybrid_backend_summary(&summary);
    assert_no_mineru_in_backend_summary(&summary);

    let chunk_count = ctx
        .query_ingested_chunk_units(&upload.document_id)
        .await
        .expect("chunk count");
    eprintln!(
        "[llm_real/pdf_corpus] ingested {chunk_count} retrievable units for doc {}",
        upload.document_id
    );
    assert!(
        chunk_count >= min_chunk_units,
        "expected >={min_chunk_units} retrievable units for {}, got {chunk_count}",
        upload.document_id
    );

    upload
}

/// Bundled PDF → LiteParse hybrid ingest → real-LLM RAG with citations.
#[tokio::test]
#[ignore = "requires real LLM + embedding; run with --ignored --test-threads=1"]
async fn real_llm_rag_bundled_pdf_corpus_query() {
    super::require_nightly_suite();
    apply_p4_pdf_ingestion_profile();
    assert!(
        bundled_pdf_path().is_file(),
        "missing bundled fixture phase0-mini.pdf"
    );

    let mut ctx = TestContext::new_with_real_llm().await;
    let path = bundled_pdf_path().to_string_lossy().to_string();
    let upload = ingest_pdf_with_routing_asserts(&mut ctx, &path, BUNDLED_INGEST_TIMEOUT, 1).await;

    let result = chat_with_citations_retry_attempts(
        &ctx,
        "According to the uploaded PDF, what is LiteParse and how is it used? Cite the document.",
        &upload.workspace_id,
        &[upload.document_id.clone()],
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS,
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_substantive(resp, 40);
    assert!(
        resp.tool_results
            .iter()
            .any(|r| RETRIEVAL_TOOLS.contains(&r.tool.as_str())),
        "expected retrieval-class tool, got: {:?}",
        resp.tool_results
            .iter()
            .map(|r| &r.tool)
            .collect::<Vec<_>>()
    );

    ctx.save_llm_artifact(
        "real_llm_rag_bundled_pdf_corpus_query",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "pdf_path": path,
                "document_id": upload.document_id,
            })),
        ),
        Some(result.reasoning),
    );
}

/// Notebook with bundled PDF + txt — multi-doc scope RAG (not MinerU-era dual-book PDF).
#[tokio::test]
#[ignore = "requires real LLM + embedding; run with --ignored --test-threads=1"]
async fn real_llm_rag_multidoc_pdf_and_txt() {
    super::require_nightly_suite();
    apply_p4_pdf_ingestion_profile();
    let pdf_path = bundled_pdf_path();
    assert!(pdf_path.is_file(), "missing phase0-mini.pdf");

    let mut ctx = TestContext::new_with_real_llm().await;
    let notebook = ctx
        .create_notebook("pdf-txt-corpus")
        .await
        .expect("notebook");

    let pdf_upload = ctx
        .upload_file_from_path_to_notebook(&pdf_path.to_string_lossy(), &notebook.id)
        .await
        .expect("upload pdf");
    let txt_upload = ctx
        .upload_document_to_notebook("antifragile.txt", &notebook.id)
        .await
        .expect("upload txt");
    assert_eq!(pdf_upload.status, 201);
    assert_eq!(txt_upload.status, 201);

    for (label, doc_id) in [
        ("pdf", pdf_upload.document_id.as_str()),
        ("txt", txt_upload.document_id.as_str()),
    ] {
        let status = ctx
            .wait_for_ingestion(doc_id, BUNDLED_INGEST_TIMEOUT)
            .await
            .unwrap_or_else(|e| panic!("ingest {label}: {e}"));
        assert_eq!(status, DocumentStatus::Completed);
    }

    let pdf_summary = ctx
        .query_latest_backend_summary(&pdf_upload.document_id)
        .await
        .expect("pdf summary");
    assert_liteparse_hybrid_backend_summary(&pdf_summary);
    assert_no_mineru_in_backend_summary(&pdf_summary);

    let doc_scope = vec![
        pdf_upload.document_id.clone(),
        txt_upload.document_id.clone(),
    ];
    let result = chat_with_citations_retry(
        &ctx,
        "Explain antifragility using the uploaded documents and cite your sources.",
        &notebook.id,
        &doc_scope,
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_answer_substantive(resp, 60);

    ctx.save_llm_artifact(
        "real_llm_rag_multidoc_pdf_and_txt",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "pdf_document_id": pdf_upload.document_id,
                "txt_document_id": txt_upload.document_id,
            })),
        ),
        Some(result.reasoning),
    );
}

/// Optional local large PDF staging probe (manual only; not a CI gate).
#[tokio::test]
#[ignore = "requires E2E_LLM_REAL_STAGING_PDF + real Paddle; manual staging only"]
async fn real_llm_rag_staging_local_book_pdf() {
    super::require_nightly_suite();
    apply_p4_pdf_ingestion_profile();
    let Some(path) = staging_pdf_path() else {
        eprintln!(
            "SKIP: real_llm_rag_staging_local_book_pdf (set E2E_LLM_REAL_STAGING_PDF to an existing PDF)"
        );
        return;
    };

    let mut ctx = TestContext::new_with_real_llm_pdf().await;
    let upload = ingest_pdf_with_routing_asserts(&mut ctx, &path, STAGING_INGEST_TIMEOUT, 2).await;

    let result = chat_with_retry(
        &ctx,
        "Summarize the author and a core idea from an early section, with citations.",
        &upload.workspace_id,
        &[upload.document_id.clone()],
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_substantive(resp, 80);

    ctx.save_llm_artifact(
        "real_llm_rag_staging_local_book_pdf",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "pdf_path": path,
                "document_id": upload.document_id,
            })),
        ),
        Some(result.reasoning),
    );
}
