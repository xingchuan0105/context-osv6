//! Real-PDF llm_real probes — use full books instead of the 1-chunk summary fixture.
//!
//! Default paths (WSL via /mnt/e):
//!   E2E_LLM_REAL_ANTIFRAGILE_PDF
//!   E2E_LLM_REAL_BLACK_SWAN_PDF
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::pdf_corpus -- --ignored --test-threads=1 --nocapture

use std::path::Path;
use std::time::Duration;

use crate::product_e2e::{
    DocumentStatus, TestContext,
    assertions::{assert_answer_substantive, assert_citation_doc_id, assert_has_citations},
    llm_real::{chat_with_citations_retry, chat_with_retry, merge_llm_real_extra},
};

const RETRIEVAL_TOOLS: &[&str] = &["dense_retrieval", "index_lookup", "doc_profile", "doc_summary"];
/// Per-document ingest wait; full books are trimmed via `INGESTION_PDF_MAX_PAGES`.
const PDF_INGEST_TIMEOUT: Duration = Duration::from_secs(1200);

/// Fast profile: cap pages and skip optional LLM side-effects (triplet/VLM summary).
fn apply_fast_pdf_ingestion_profile() {
    let max_pages = std::env::var("E2E_PDF_MAX_PAGES").unwrap_or_else(|_| "80".to_string());
    unsafe {
        std::env::set_var("INGESTION_PDF_MAX_PAGES", &max_pages);
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_TRIPLET_ENABLED", "0");
        std::env::set_var("INGESTION_VLM_SUMMARY_ENABLED", "0");
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub(crate) fn antifragile_pdf_path() -> String {
    env_or_default(
        "E2E_LLM_REAL_ANTIFRAGILE_PDF",
        "/mnt/e/OneDrive/桌面/知境笔记/Taleb_Antifragile__2012.pdf",
    )
}

pub(crate) fn black_swan_pdf_path() -> String {
    env_or_default(
        "E2E_LLM_REAL_BLACK_SWAN_PDF",
        "/mnt/e/OneDrive/桌面/知境笔记/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf",
    )
}

fn require_pdf(path: &str, env_key: &str) {
    assert!(
        Path::new(path).is_file(),
        "PDF not found at {path}. Set {env_key} to an existing file."
    );
}

async fn require_service_health(base_url: &str, service_name: &str, start_hint: &str) {
    let health = format!("{}/v1/healthz", base_url.trim_end_matches('/'));
    let ok = reqwest::Client::new()
        .get(&health)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    assert!(
        ok,
        "{service_name} must be running (expected {health}). Start: {start_hint}"
    );
}

async fn require_pdf_ingestion_prereqs() {
    let office_url = std::env::var("OFFICE_PARSER_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());
    require_service_health(
        &office_url,
        "office-parser-jvm",
        "./scripts/office-parser-up.sh",
    )
    .await;

    let renderer_url = std::env::var("PDF_RENDERER_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9091".to_string());
    require_service_health(
        &renderer_url,
        "pdf-visual-renderer",
        "./scripts/pdf-renderer-up.sh",
    )
    .await;
}

async fn ingest_pdf(ctx: &mut TestContext, path: &str, env_key: &str) -> crate::product_e2e::UploadResponse {
    require_pdf(path, env_key);
    eprintln!("[llm_real/pdf] uploading {}", path);
    let upload = ctx
        .upload_file_from_path(path)
        .await
        .expect("upload pdf");
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, PDF_INGEST_TIMEOUT)
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
            "PDF ingestion failed for {}: status={status:?}, detail={}. \
             See tests/e2e_output/failures/real_llm_pdf_ingestion_failed/worker_logs.txt \
             and worker_log_tail below.\n{worker_tail}",
            upload.document_id,
            serde_json::to_string_pretty(&detail).unwrap_or_default()
        );
    }

    let chunk_count = ctx
        .query_ingested_chunk_units(&upload.document_id)
        .await
        .expect("chunk count");
    eprintln!(
        "[llm_real/pdf] ingested {} retrievable units for doc {}",
        chunk_count, upload.document_id
    );
    assert!(
        chunk_count > 1,
        "expected multi-chunk PDF corpus, got {chunk_count} retrievable units for {}",
        upload.document_id
    );

    upload
}

/// R1 on full Antifragile PDF — reproduces the prior single-tool / no-TOC issue probe.
#[tokio::test]
#[ignore = "requires real LLM + local PDF; run with --ignored --test-threads=1"]
async fn real_llm_rag_complex_query_antifragile_pdf() {

    super::require_nightly_suite();
    apply_fast_pdf_ingestion_profile();
    require_pdf_ingestion_prereqs().await;
    let mut ctx = TestContext::new_with_real_llm_pdf().await;
    let pdf = antifragile_pdf_path();
    let upload = ingest_pdf(&mut ctx, &pdf, "E2E_LLM_REAL_ANTIFRAGILE_PDF").await;

    let result = chat_with_retry(
        &ctx,
        "First summarize this book's author and chapter structure, then explain a core idea from an early section.",
        &upload.notebook_id,
        &[upload.document_id.clone()],
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_substantive(resp, 80);
    assert!(
        resp.tool_results.iter().any(|r| RETRIEVAL_TOOLS.contains(&r.tool.as_str())),
        "expected retrieval-class tool, got: {:?}",
        resp.tool_results.iter().map(|r| &r.tool).collect::<Vec<_>>()
    );

    let distinct_tools: std::collections::HashSet<_> =
        resp.tool_results.iter().map(|r| r.tool.as_str()).collect();
    let chunk_ids: std::collections::HashSet<_> = resp
        .citations
        .iter()
        .filter_map(|c| c.chunk_id.as_deref())
        .collect();
    eprintln!(
        "[llm_real/pdf] distinct_tools={distinct_tools:?} distinct_citation_chunks={}",
        chunk_ids.len()
    );
    if distinct_tools.len() < 2 {
        eprintln!(
            "[llm_real/pdf] multi-tool goal not met — inspect reasoning_summary.txt artifact"
        );
    }

    ctx.save_llm_artifact(
        "real_llm_rag_complex_query_antifragile_pdf",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "pdf_path": pdf,
                "document_id": upload.document_id,
                "distinct_tools": resp.tool_results.iter().map(|r| &r.tool).collect::<Vec<_>>(),
                "distinct_citation_chunks": chunk_ids.len(),
                "multitool_goal_met": distinct_tools.len() >= 2,
            })),
        ),
        Some(result.reasoning),
    );
}

/// Two-book notebook: Antifragile scope + question referencing Black Swan themes.
#[tokio::test]
#[ignore = "requires real LLM + local PDFs; run with --ignored --test-threads=1"]
async fn real_llm_rag_multidoc_antifragile_and_black_swan_pdf() {

    super::require_nightly_suite();
    apply_fast_pdf_ingestion_profile();
    require_pdf_ingestion_prereqs().await;
    let mut ctx = TestContext::new_with_real_llm_pdf().await;
    let antifragile = antifragile_pdf_path();
    let black_swan = black_swan_pdf_path();

    let notebook = ctx.create_notebook("pdf-corpus").await.expect("notebook");
    let upload_a = ctx
        .upload_file_from_path_to_notebook(&antifragile, &notebook.id)
        .await
        .expect("upload antifragile");
    let upload_b = ctx
        .upload_file_from_path_to_notebook(&black_swan, &notebook.id)
        .await
        .expect("upload black swan");
    assert_eq!(upload_a.status, 201);
    assert_eq!(upload_b.status, 201);

    for (label, doc_id) in [
        ("antifragile", upload_a.document_id.as_str()),
        ("black_swan", upload_b.document_id.as_str()),
    ] {
        let status = ctx
            .wait_for_ingestion(doc_id, PDF_INGEST_TIMEOUT)
            .await
            .unwrap_or_else(|e| panic!("ingest {label}: {e}"));
        assert_eq!(status, DocumentStatus::Completed);
        let n = ctx.query_ingested_chunk_units(doc_id)
            .await
            .expect("chunks");
        eprintln!("[llm_real/pdf] {label}: {n} retrievable units");
        assert!(n > 1, "{label} should produce multiple retrievable units");
    }

    let doc_scope = vec![upload_a.document_id.clone(), upload_b.document_id.clone()];
    let query = "Using the Antifragile document, explain what antifragility means with citations. \
                 If retrieval includes page-level chunks from The Black Swan PDF, cite at least one \
                 such page as well.";
    let result = chat_with_citations_retry(&ctx, query, &notebook.id, &doc_scope).await;
    let resp = &result.resp;

    let cited_docs: std::collections::HashSet<_> = resp.citations.iter().map(|c| &c.doc_id).collect();
    eprintln!(
        "[llm_real/pdf] multidoc cited_docs={} tools={:?} degrade={:?}",
        cited_docs.len(),
        resp.tool_results.iter().map(|r| &r.tool).collect::<Vec<_>>(),
        resp.degrade_trace
    );

    ctx.save_llm_artifact(
        "real_llm_rag_multidoc_antifragile_and_black_swan_pdf",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "antifragile_pdf": antifragile,
                "black_swan_pdf": black_swan,
                "document_ids": doc_scope,
                "cited_doc_count": cited_docs.len(),
                "distinct_tools": resp.tool_results.iter().map(|r| &r.tool).collect::<Vec<_>>(),
            })),
        ),
        Some(result.reasoning),
    );

    assert_has_citations(resp);
    assert_answer_substantive(resp, 80);
    assert!(
        cited_docs.contains(&upload_a.document_id),
        "expected citation from Antifragile doc {}",
        upload_a.document_id
    );
}
