//! PR-6 Step A: `ProductionRagEvaluator` — REAL `RagRuntime` (planner / RRF / re-rank with
//! real embeddings) against the golden set's RAG subset, via the product_e2e chat flow.
//!
//! Production-grade counterpart to `tests/rag_quality/src/bin/quality_runner.rs` (the smoke
//! evaluator). The smoke runner uses single-pass flat-cosine over the product_e2e MOCK
//! embedding server, which returns IDENTICAL vectors (`mock_embedding_server.rs`:
//! "All vectors identical so dense retrieval always returns high similarity") — so its
//! "recall" is noise, not the product. This evaluator instead drives
//! `TestContext::new_with_real_llm()` — real DashScope `text-embedding-v4` for retrieval +
//! a real chat LLM for synthesis — so `ChatResponse.tool_results` carry the chunks the real
//! `RagRuntime` returned after planning + hybrid retrieval + re-rank. Recall@15 is scored
//! against that **retrieval layer** (`extract_retrieved_chunks`), decoupled from the
//! synthesizer's citation selection (`ChatResponse.citations`), which is what a blocking
//! release gate must measure.
//!
//! The answer's `[[N]]` citation markup (from `materialize_answer_markup`) is rewritten to
//! `[citation:N]` so `EvaluationMetrics::extract_citation_indices` (regex `\[citation:(\d)\]`)
//! can score it. Hallucination is the word-overlap heuristic — see
//! `tests/rag_quality/GOTCHAS.md`: 15-30% false positives, noise until replaced with NLI.
//! This test REPORTS hallucination but does NOT gate on it; Step B's release gate gates on
//! recall + citation only until NLI lands.
//!
//! NOTE: this bypasses `rag_quality::EvaluationHarness` (the `RagEvaluator` trait requires
//! `Send` futures, but `TestContext` holds `oneshot::Sender`s and is not `Sync`, so a future
//! borrowing `&TestContext` cannot be `Send`). We reuse `EvaluationMetrics` directly — the
//! metrics are the valuable part; the harness is just a loop wrapper that we inline here.
//!
//! `#[ignore]` because it incurs real LLM/embedding API cost + is non-deterministic.
//! Run locally (Milvus + PG up, `avrag-rs/.env` has real `AGENT_LLM_*` + `EMBEDDING_*`):
//! ```bash
//! E2E_MODE=nightly cargo test -p app --test product_e2e rag_quality_prod \
//!   --features product-e2e -- --ignored --test-threads=1 --nocapture
//! ```

use std::io::Write;
use std::time::Duration;

use rag_quality::{
    EvaluationMetrics, GoldenDataset, GoldenExample, ScorecardSummary, ToolCoverageScore,
    ToolCoverageSummary, extract_cited_chunks, extract_retrieved_chunks, extract_tool_trace,
    score_query,
};
use regex::Regex;

use super::{
    ObservabilityMode, chat_rag_observable_probe, count_sse_trace_stage, summarize_tool_activity,
};
use crate::product_e2e::fixtures::shared_smoke_v5_context;
use crate::product_e2e::{ChatResponse, DocumentStatus, TestContext};

/// Rewrite the production answer's citation markup to `[citation:N]` so
/// `EvaluationMetrics::extract_citation_indices` (regex `\[citation:(\d+)\]`) can read it.
///
/// The non-streaming chat response carries the RAW LLM markup `[[cite:CHUNK_ID]]` (the
/// streaming path's `materialize_answer_markup` is NOT applied to non-streaming responses),
/// so we map each `[[cite:CHUNK_ID]]` to `[citation:{citation_id}]` via the response's
/// `citations` (chunk_id → 1-based citation_id). Any pre-materialized `[[N]]` (numeric) is
/// also rewritten; `[[image:...]]` is left untouched (image citations aren't in the rag subset).
fn rewrite_citations(
    answer: &str,
    chunk_to_cite: &std::collections::HashMap<String, i64>,
) -> String {
    let cite_re = Regex::new(r"\[\[cite:([^\]]+)\]\]").expect("cite rewrite regex");
    let after_cite = cite_re.replace_all(answer, |caps: &regex::Captures| {
        let chunk_id = caps.get(1).unwrap().as_str().trim().to_string();
        match chunk_to_cite.get(&chunk_id) {
            Some(n) => format!("[citation:{n}]"),
            None => String::new(),
        }
    });
    let num_re = Regex::new(r"\[\[(\d+)\]\]").expect("numeric citation rewrite regex");
    num_re
        .replace_all(&after_cite, "[citation:$1]")
        .into_owned()
}

fn print_tool_coverage_summary(title: &str, summary: &ToolCoverageSummary) {
    eprintln!();
    eprintln!("{title}");
    eprintln!(
        "  tool_coverage: {:.1}% ({}/{}) single_tool={:.1}% ({}/{}) sequence={:.1}% ({}/{})",
        summary.coverage_rate * 100.0,
        summary.covered,
        summary.with_expectations,
        summary.single_tool_hit_rate * 100.0,
        summary.single_tool_hit,
        summary.single_tool_total,
        summary.sequence_hit_rate * 100.0,
        summary.sequence_hit,
        summary.sequence_total,
    );
    if summary.triplet_reingest_pending > 0 {
        eprintln!(
            "  triplet_reingest probes: {}/{} covered (need INGESTION_TRIPLET_ENABLED=1 re-ingest)",
            summary.triplet_reingest_covered, summary.triplet_reingest_pending
        );
    }
}

fn print_scorecard_summary(title: &str, summary: &ScorecardSummary) {
    eprintln!();
    eprintln!("{title}");
    eprintln!(
        "  retrieval: recall@15={:.2}% hit@15={:.2}% mrr={:.3} ndcg@15={:.3}",
        summary.retrieval_recall_at_k * 100.0,
        summary.retrieval_hit_at_k * 100.0,
        summary.retrieval_mrr,
        summary.retrieval_ndcg
    );
    eprintln!(
        "  retrieval(graded): graded_recall@15={:.2}% graded_ndcg@15={:.3}",
        summary.retrieval_graded_recall_at_k * 100.0,
        summary.retrieval_graded_ndcg
    );
    eprintln!(
        "  retrieval(answerable-only, excl. vacuous adversarial 100%): recall@15={:.2}% graded_recall@15={:.2}% substring_faithfulness={:.2}%",
        summary.retrieval_recall_at_k_on_answerable * 100.0,
        summary.retrieval_graded_recall_at_k_on_answerable * 100.0,
        summary.faithfulness_mean_on_answerable * 100.0
    );
    eprintln!(
        "  selection: precision={:.2}% recall={:.2}%",
        summary.selection_precision * 100.0,
        summary.selection_recall * 100.0
    );
    eprintln!(
        "  generation: refusal_correct={:.2}% contract={:.2}% substring_faithfulness={:.2}%",
        summary.refusal_correct_rate * 100.0,
        summary.contract_compliance_rate * 100.0,
        summary.faithfulness_mean * 100.0
    );
    let labels = summary
        .label_counts
        .iter()
        .map(|(label, count)| format!("{}={count}", label.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!("  labels: {labels}");
}

#[derive(Debug, Clone)]
struct SmokeScorecardRow {
    subset: String,
    query: String,
    label: String,
    retrieval_recall: f64,
    selection_precision: f64,
    faithfulness: f64,
}

fn append_smoke_loop_scorecard(
    summary: &ScorecardSummary,
    rows: &[SmokeScorecardRow],
) -> std::io::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("prompts/_backups/loop_iterations.md");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let labels = summary
        .label_counts
        .iter()
        .map(|(label, count)| format!("{}={count}", label.as_str()))
        .collect::<Vec<_>>()
        .join(", ");

    writeln!(file)?;
    writeln!(file, "---")?;
    writeln!(file)?;
    writeln!(file, "## Smoke v5 decoupled scorecard (auto)")?;
    writeln!(file)?;
    writeln!(
        file,
        "**Retrieval:** recall@15={:.2}% | hit@15={:.2}% | mrr={:.3} | ndcg@15={:.3}",
        summary.retrieval_recall_at_k * 100.0,
        summary.retrieval_hit_at_k * 100.0,
        summary.retrieval_mrr,
        summary.retrieval_ndcg
    )?;
    writeln!(
        file,
        "**Retrieval (graded, ADR 0011):** graded_recall@15={:.2}% | graded_ndcg@15={:.3}",
        summary.retrieval_graded_recall_at_k * 100.0,
        summary.retrieval_graded_ndcg
    )?;
    writeln!(
        file,
        "**Retrieval (answerable-only, excl. vacuous adversarial 100%):** recall@15={:.2}% | graded_recall@15={:.2}% | substring_faithfulness={:.2}%",
        summary.retrieval_recall_at_k_on_answerable * 100.0,
        summary.retrieval_graded_recall_at_k_on_answerable * 100.0,
        summary.faithfulness_mean_on_answerable * 100.0
    )?;
    writeln!(
        file,
        "**Selection:** precision={:.2}% | recall={:.2}%",
        summary.selection_precision * 100.0,
        summary.selection_recall * 100.0
    )?;
    writeln!(
        file,
        "**Generation:** refusal_correct={:.2}% | contract={:.2}% | substring_faithfulness={:.2}%",
        summary.refusal_correct_rate * 100.0,
        summary.contract_compliance_rate * 100.0,
        summary.faithfulness_mean * 100.0
    )?;
    writeln!(file)?;
    writeln!(file, "**Labels:** {labels}")?;
    writeln!(file)?;
    writeln!(
        file,
        "| subset | label | ret_recall | sel_precision | faithfulness | query |"
    )?;
    writeln!(file, "|---|---:|---:|---:|---:|---|")?;
    for row in rows {
        writeln!(
            file,
            "| {} | {} | {:.0}% | {:.0}% | {:.0}% | {} |",
            row.subset.replace('|', "\\|"),
            row.label,
            row.retrieval_recall * 100.0,
            row.selection_precision * 100.0,
            row.faithfulness * 100.0,
            row.query.replace('|', "\\|").replace('\n', " "),
        )?;
    }
    Ok(())
}

/// Keep only the `rag` subset. The chat / search subsets need a different `agent_type`
/// (the product chat helper hardcodes `agent_type=rag`), and the RAG quality gate is about
/// RAG retrieval + citation grounding — the `rag` subset is what carries signal here.
fn filter_to_rag_subset(mut ds: GoldenDataset) -> GoldenDataset {
    ds.subsets.retain(|s| s.name == "rag");
    ds
}

/// Subset label embedded in smoke probe `description` as `{subset} — {intent}`.
fn smoke_probe_subset_label(example: &GoldenExample) -> &str {
    example
        .description
        .split('—')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
}

fn smoke_probe_artifact_key(probe_index: usize, example: &GoldenExample) -> String {
    let subset = smoke_probe_subset_label(example);
    format!("{:02}_{subset}", probe_index + 1)
}

/// PR-6 Step A: prove the production evaluator runs the real `RagRuntime` (real embeddings)
/// against the golden set and emits REAL Recall@15 / Citation / Hallucination numbers — not
/// the smoke flat-cosine / mock-embedding numbers. Soft gate for Step A: recall must be
/// meaningful (>50%) and no eval failures. Step B calibrates a real baseline + makes the
/// gate blocking (recall drop + citation accuracy).
#[tokio::test]
#[ignore = "requires real LLM + embedding API keys; run with --ignored --test-threads=1"]
async fn production_rag_evaluator_runs_real_retrieval_against_golden_set() {
    super::require_nightly_suite();

    let mut ctx = TestContext::new_with_real_llm().await;
    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload antifragile fixture");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(300))
        .await
        .expect("wait for ingestion");
    assert_eq!(
        status,
        DocumentStatus::Completed,
        "ingestion should complete before evaluation"
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rag_quality/fixtures_golden.json");
    let dataset = filter_to_rag_subset(GoldenDataset::load(&path).expect("load golden set"));
    let examples: Vec<&GoldenExample> = dataset.all_examples().take(5).collect();
    eprintln!(
        "[rag_quality_prod] rag subset examples (capped at 5): {}",
        examples.len()
    );
    assert!(
        !examples.is_empty(),
        "golden set has no rag subset examples"
    );

    let notebook_id = &upload.notebook_id;
    let doc_scope = [upload.document_id.clone()];

    let mut recall_results = Vec::new();
    let mut citation_results = Vec::new();
    let mut hallucination_results = Vec::new();
    let mut scorecards = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    for example in &examples {
        let resp = match ctx.chat(&example.query, notebook_id, &doc_scope).await {
            Ok(r) => r,
            Err(e) => {
                failures.push((example.query.clone(), format!("chat: {e}")));
                continue;
            }
        };
        let chat: ChatResponse = match resp.into_business() {
            Ok(c) => c,
            Err(e) => {
                failures.push((example.query.clone(), format!("parse: {e}")));
                continue;
            }
        };
        let retrieved = extract_retrieved_chunks(&chat.tool_results);
        let cited = extract_cited_chunks(&chat.citations);
        let chunks: Vec<String> = retrieved.contents();
        let chunk_to_cite: std::collections::HashMap<String, i64> = chat
            .citations
            .iter()
            .filter_map(|c| c.chunk_id.clone().map(|id| (id, c.citation_id)))
            .collect();
        let answer = rewrite_citations(&chat.answer, &chunk_to_cite);
        eprintln!(
            "[rag_quality_prod] Q={:?} retrieved_chunks={} answer_len={}",
            example.query,
            chunks.len(),
            chat.answer.len()
        );

        let citation_indices = EvaluationMetrics::extract_citation_indices(&answer);
        let recall = EvaluationMetrics::recall_at_k(&example.query, &chunks, example, 15);
        let citation =
            EvaluationMetrics::citation_accuracy(&example.query, &citation_indices, example);
        let halluc = EvaluationMetrics::hallucination_check(&example.query, &answer, &chunks);
        let scorecard = score_query(&retrieved, &cited, &answer, example, 15);
        eprintln!(
            "    recall@15={:.0}% ({}/{} matched) cit_acc={:.0}% (tp={} missing={:?}) halluc_score={:.2} label={}",
            recall.recall * 100.0,
            recall.matched_chunks.len(),
            recall.golden_count,
            citation.accuracy * 100.0,
            citation.true_positives,
            citation.missing,
            halluc.hallucination_score,
            scorecard.label.as_str()
        );
        eprintln!(
            "    markup_diag: [[count={} [[cite:count={} [citation:count={} rewritten_idx={:?} preview={:?}",
            chat.answer.matches("[[").count(),
            chat.answer.matches("[[cite:").count(),
            chat.answer.matches("[citation:").count(),
            citation_indices,
            chat.answer.chars().take(400).collect::<String>(),
        );
        recall_results.push(recall);
        citation_results.push(citation);
        hallucination_results.push(halluc);
        scorecards.push(scorecard);
    }

    let metrics =
        EvaluationMetrics::aggregate(recall_results, citation_results, hallucination_results);
    let scorecard_summary = ScorecardSummary::from_scorecards(&scorecards);

    eprintln!();
    eprintln!("=========================================");
    eprintln!("Production RAG Quality Report (real RagRuntime)");
    eprintln!("=========================================");
    eprintln!("Total examples:      {}", metrics.total_examples);
    eprintln!(
        "Recall@15:           {:.2}%  (real retrieval)",
        metrics.recall_at_15 * 100.0
    );
    eprintln!(
        "Citation Accuracy:   {:.2}%",
        metrics.citation_accuracy * 100.0
    );
    eprintln!(
        "Hallucination Rate:  {:.2}%  (heuristic — noise until NLI; not gated)",
        metrics.hallucination_rate * 100.0
    );
    print_scorecard_summary(
        "Decoupled RAG Scorecard (retrieval / selection / generation)",
        &scorecard_summary,
    );
    if !failures.is_empty() {
        eprintln!("Failures ({}):", failures.len());
        for (q, err) in &failures {
            eprintln!("  - {q:?}: {err}");
        }
    }
    // Blocking gate. Retrieval-layer Recall@15 must not drop more than 3% from the calibrated
    // baseline (0.80). Q1 ("Who developed antifragility") is a known retrieval-hard case:
    // its golden chunk is the terse author line "Nassim Nicholas Taleb", whose embedding
    // rarely surfaces for the conceptual query, so the agent refuses (0 chunks). Q2–Q5
    // reliably retrieve the single rich chunk (antifragile.txt is small → one chunk holds
    // all concepts). The 0.80 lower bound avoids flaking on Q1 while still catching
    // regressions on Q2–Q5. Generation gates are refusal_correct=100% and
    // contract_compliance=100%. Citation precision and substring faithfulness are reported
    // while calibration / LLM-Judge work continues.
    const RECALL_BASELINE: f64 = 0.80;
    eprintln!();
    eprintln!(
        "Step B gate — BLOCKING: recall drop ≤3% from baseline {:.0}%, \
         refusal_correct=100%, contract=100%. Citation/faithfulness reported, not gated yet.",
        RECALL_BASELINE * 100.0
    );
    eprintln!(
        "  Legacy recall-gate reference (assert_passing): {:?}",
        metrics.assert_passing(RECALL_BASELINE)
    );

    assert!(metrics.total_examples > 0, "should have run rag examples");
    assert!(failures.is_empty(), "eval failures: {failures:?}");
    let recall_drop = RECALL_BASELINE - metrics.recall_at_15;
    assert!(
        recall_drop <= 0.03,
        "Recall@15 regression: {:.1}% drop (gate: ≤3% from baseline {:.0}%). \
         Current: {:.2}%. Citation/faithfulness reported, not gated — see GOTCHAS.md.",
        recall_drop * 100.0,
        RECALL_BASELINE * 100.0,
        metrics.recall_at_15 * 100.0,
    );
    assert!(
        (scorecard_summary.refusal_correct_rate - 1.0).abs() < f64::EPSILON,
        "Refusal correctness gate failed: {:.2}%",
        scorecard_summary.refusal_correct_rate * 100.0,
    );
    assert!(
        (scorecard_summary.contract_compliance_rate - 1.0).abs() < f64::EPSILON,
        "Contract compliance gate failed: {:.2}%",
        scorecard_summary.contract_compliance_rate * 100.0,
    );
}

/// Realistic-corpus production evaluator: runs the full 107-example golden set
/// (`golden_set_realistic.json`) against 7 real private documents (TXT/MD).
///
/// This is a **calibration run** — it reports Recall@15, Citation Accuracy, and
/// Hallucination Rate but does NOT gate, because the baseline for the new corpus
/// has not been calibrated yet. After 2~3 stable runs, set `RECALL_BASELINE` to
/// the observed mean and switch the assert to a blocking gate.
///
/// Corpus (all in `tests/product_e2e/fixtures/`):
/// - `thesis_y_refrigeration.txt` — MBA thesis, 52K chars (DOCX converted to TXT)
/// - `adr-0004-rag-agent-loop.md` — ADR, 541 words
/// - `adr-0009-codegen-sandbox-bridge.md` — ADR, 1K chars
/// - `consulting_platform_network_effects.txt` — consulting article, 18K chars
/// - `consulting_compensation_design.txt` — compensation article, 3K chars
/// - `huawei_ipd_370_activities.txt` — IPD spreadsheet as TSV, 54K chars
/// - `baiyao_it_planning.txt` — IT planning PDF converted to TXT, 20K chars
///
/// All 7 files are uploaded to a single notebook so cross-document queries work.
/// `doc_scope` includes all 7 document IDs.
///
/// Note: DOCX/XLSX/PDF were converted to TXT for this calibration run because
/// the office parser service (port 9090) was not running at test time. The TXT
/// files preserve full text content for retrieval quality testing. To test the
/// full multimodal pipeline (image summaries, KG triplets), start the office
/// parser (`scripts/office-parser-up.sh`) and Paddle OCR, then switch the
/// corpus list back to the original DOCX/XLSX/PDF files.
///
/// Run with:
/// ```bash
/// E2E_MODE=nightly cargo test -p app --test product_e2e realistic_corpus \
///   --features product-e2e -- --ignored --test-threads=1 --nocapture
/// ```
#[tokio::test]
#[ignore = "requires real LLM + embedding API keys; run with --ignored --test-threads=1"]
async fn realistic_corpus_full_eval() {
    super::require_nightly_suite();

    // Use the PDF profile for longer ingestion timeout (large corpus).
    let mut ctx = TestContext::new_with_real_llm_pdf().await;

    // --- Upload all 7 corpus files to a single notebook ---
    let notebook = ctx
        .create_notebook("rag-quality-realistic-corpus")
        .await
        .expect("create notebook");
    let notebook_id = notebook.id.clone();

    let corpus_files = [
        ("thesis_y_refrigeration.txt", 600),         // 52K chars, thesis
        ("adr-0004-rag-agent-loop.md", 120),         // 4.8KB MD
        ("adr-0009-codegen-sandbox-bridge.md", 120), // 13.6KB MD
        ("consulting_platform_network_effects.txt", 300), // 18K chars
        ("consulting_compensation_design.txt", 120), // 3K chars
        ("huawei_ipd_370_activities.txt", 120),      // 54K chars, table as TSV
        ("baiyao_it_planning.txt", 300),             // 20K chars, PDF->TXT
    ];

    let mut doc_ids: Vec<String> = Vec::new();
    for (filename, timeout_secs) in &corpus_files {
        eprintln!("[realistic_corpus] uploading {filename} ...");
        let upload = ctx
            .upload_document_to_notebook(filename, &notebook_id)
            .await
            .unwrap_or_else(|e| panic!("upload {filename}: {e}"));
        let status = ctx
            .wait_for_ingestion(&upload.document_id, Duration::from_secs(*timeout_secs))
            .await
            .unwrap_or_else(|e| panic!("wait_for_ingestion {filename}: {e}"));
        assert_eq!(
            status,
            DocumentStatus::Completed,
            "ingestion failed for {filename}"
        );
        eprintln!(
            "[realistic_corpus] {filename} ingested (doc_id={})",
            upload.document_id
        );
        doc_ids.push(upload.document_id);
    }
    assert_eq!(doc_ids.len(), 7, "should have 7 documents ingested");

    // --- Load the realistic golden set ---
    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rag_quality/golden_set_realistic.json");
    let dataset = GoldenDataset::load(&golden_path).expect("load realistic golden set");
    let examples: Vec<&GoldenExample> = dataset.all_examples().collect();
    eprintln!(
        "[realistic_corpus] golden set v{}: {} examples across {} subsets",
        dataset.version,
        examples.len(),
        dataset.subsets.len()
    );
    assert!(!examples.is_empty(), "golden set is empty");

    // --- Run evaluation ---
    let doc_scope: Vec<String> = doc_ids.clone();
    let mut recall_results = Vec::new();
    let mut citation_results = Vec::new();
    let mut hallucination_results = Vec::new();
    let mut scorecards = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut per_subset_stats: std::collections::HashMap<String, (usize, usize, f64)> =
        std::collections::HashMap::new();

    for (idx, example) in examples.iter().enumerate() {
        let subset_name = dataset
            .subsets
            .iter()
            .find(|s| s.examples.iter().any(|e| e.query == example.query))
            .map(|s| s.name.as_str())
            .unwrap_or("unknown");

        eprintln!(
            "\n[realistic_corpus] {}/{} subset={} Q={:?}",
            idx + 1,
            examples.len(),
            subset_name,
            example.query.chars().take(60).collect::<String>()
        );

        // Use non-streaming mode (stream:false). The streaming path has a
        // "missing done payload" issue with Chinese queries on some LLM providers.
        // Non-streaming mode runs the full RAG agent loop and returns a complete JSON
        // response — some queries may return a degrade response without an `answer`
        // field, which we catch and record as a failure with the raw JSON for debugging.
        let resp = match ctx
            .chat_without_mock_chunk_pin(&example.query, &notebook_id, &doc_scope)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                failures.push((example.query.clone(), format!("chat: {e}")));
                eprintln!("  FAIL: chat error: {e}");
                continue;
            }
        };
        let resp_status = resp.status;
        let resp_body = resp.body_json.clone();
        let chat: ChatResponse = match resp.into_business() {
            Ok(c) => c,
            Err(e) => {
                let raw = serde_json::to_string_pretty(&resp_body)
                    .unwrap_or_else(|_| "<serialize failed>".to_string());
                failures.push((example.query.clone(), format!("parse: {e}")));
                eprintln!("  FAIL: parse error: {e}");
                eprintln!(
                    "  raw response (status={}): {}",
                    resp_status,
                    &raw[..raw.len().min(500)]
                );
                continue;
            }
        };
        let retrieved = extract_retrieved_chunks(&chat.tool_results);
        let cited = extract_cited_chunks(&chat.citations);
        let chunks: Vec<String> = retrieved.contents();
        let chunk_to_cite: std::collections::HashMap<String, i64> = chat
            .citations
            .iter()
            .filter_map(|c| c.chunk_id.clone().map(|id| (id, c.citation_id)))
            .collect();
        let answer = rewrite_citations(&chat.answer, &chunk_to_cite);

        let citation_indices = EvaluationMetrics::extract_citation_indices(&answer);
        let recall = EvaluationMetrics::recall_at_k(&example.query, &chunks, example, 15);
        let citation =
            EvaluationMetrics::citation_accuracy(&example.query, &citation_indices, example);
        let halluc = EvaluationMetrics::hallucination_check(&example.query, &answer, &chunks);
        let scorecard = score_query(&retrieved, &cited, &answer, example, 15);

        eprintln!(
            "  recall@15={:.0}% ({}/{}) cit_acc={:.0}% (tp={} missing={:?}) halluc={:.2} chunks={} ans_len={} label={}",
            recall.recall * 100.0,
            recall.matched_chunks.len(),
            recall.golden_count,
            citation.accuracy * 100.0,
            citation.true_positives,
            citation.missing,
            halluc.hallucination_score,
            chunks.len(),
            chat.answer.len(),
            scorecard.label.as_str()
        );

        // Track per-subset stats: (count, matched_count, sum_recall)
        let entry = per_subset_stats
            .entry(subset_name.to_string())
            .or_insert((0, 0, 0.0));
        entry.0 += 1;
        if recall.recall >= 1.0 {
            entry.1 += 1;
        }
        entry.2 += recall.recall;

        recall_results.push(recall);
        citation_results.push(citation);
        hallucination_results.push(halluc);
        scorecards.push(scorecard);
    }

    // --- Aggregate and report ---
    let metrics =
        EvaluationMetrics::aggregate(recall_results, citation_results, hallucination_results);
    let scorecard_summary = ScorecardSummary::from_scorecards(&scorecards);

    eprintln!();
    eprintln!("=========================================");
    eprintln!("Realistic Corpus RAG Quality Report (real RagRuntime)");
    eprintln!("=========================================");
    eprintln!("Golden set version:  {}", dataset.version);
    eprintln!("Corpus:              7 documents (TXT/MD)");
    eprintln!("Total examples:      {}", metrics.total_examples);
    eprintln!("Recall@15:           {:.2}%", metrics.recall_at_15 * 100.0);
    eprintln!(
        "Citation Accuracy:   {:.2}%",
        metrics.citation_accuracy * 100.0
    );
    eprintln!(
        "Hallucination Rate:  {:.2}%  (heuristic — noise until NLI; not gated)",
        metrics.hallucination_rate * 100.0
    );
    print_scorecard_summary(
        "Decoupled RAG Scorecard (retrieval / selection / generation)",
        &scorecard_summary,
    );

    eprintln!();
    eprintln!("Per-subset breakdown:");
    eprintln!(
        "  {:<25} {:>6} {:>8} {:>10}",
        "subset", "count", "matched", "avg_recall"
    );
    for s in &dataset.subsets {
        if let Some(&(count, matched, sum_recall)) = per_subset_stats.get(&s.name) {
            eprintln!(
                "  {:<25} {:>6} {:>8} {:>9.1}%",
                s.name,
                count,
                matched,
                (sum_recall / count as f64) * 100.0
            );
        }
    }

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failures ({}):", failures.len());
        for (q, err) in &failures {
            eprintln!("  - {:?}: {}", q.chars().take(50).collect::<String>(), err);
        }
    }

    eprintln!();
    eprintln!("NOTE: This is a CALIBRATION RUN — no blocking gate.");
    eprintln!("After 2~3 stable runs, set RECALL_BASELINE to the observed mean");
    eprintln!("and enable the blocking assert below.");

    // Calibration run — no recall gate. We just report numbers.
    // After 2~3 stable runs, set RECALL_BASELINE and enable the blocking assert.
    assert!(metrics.total_examples > 0, "should have run examples");
    assert!(
        failures.len() < examples.len(),
        "all examples failed — check service health. Failures: {failures:?}"
    );
}

/// Smoke eval for the v5 `rag-system.md` prompt (agent-centered ReAct: information-gap
/// framing, budget-aware, A/B/C/D action selection, three-state evidence assessment).
///
/// This is NOT a full golden-set run — it is a regression probe against a cached
/// 3-document sub-corpus (thesis + IPD table + Baiyao PDF→TXT) using the curated
/// `tests/rag_quality/golden_set_smoke_v5.json` (12 probes, ~6–12 min with observability).
///
/// Probe mix (see JSON for exact queries):
/// - **Fast factual**: thesis year, 4R dimensions, 4A architecture, PAC-20
/// - **Structured / PDF**: PAC-05 row lookup, 11/100/638 nested counts
/// - **Synthesis / numeric**: 2019–2020 revenue + loss, buried 1467亿 industry size
/// - **Cross-doc**: 4R vs 4A disambiguation; IPD 370 activities vs 638 business objects
/// - **Adversarial**: warranty period (half-in-corpus), registered capital (absent)
///
/// No blocking gate (no calibrated baseline). Reports per-query recall@15 / citation /
/// hallucination / chunk count / answer preview for manual review.
///
/// Corpus reuse: first run ingests 3 documents into persistent PG/object-store/Milvus
/// and writes `crates/app/tests/e2e_output/rag_quality_smoke_v5_corpus.json`.
/// Subsequent `cargo test` runs skip ingestion when the cache is valid. Set
/// `RAG_QUALITY_SMOKE_FORCE_INGEST=1` to force a fresh ingest.
///
/// Observability artifacts (streaming + `debug: true`, per probe query):
/// `crates/app/tests/e2e_output/rag_quality_smoke_v5/{run_id}/{subset_name}/`
///   - `response.json`, `sse_events.jsonl`, `trace_reasoning.jsonl`,
///     `prompt_snapshots.json`, `reasoning_summary.txt`, `metadata.json`
///
/// Run with:
/// ```bash
/// E2E_MODE=nightly cargo test -p app --test product_e2e rag_system_prompt_smoke_v5 \
///   --features product-e2e -- --ignored --test-threads=1 --nocapture
/// ```
#[tokio::test]
#[ignore = "requires real LLM + embedding API keys; run with --ignored --test-threads=1"]
async fn rag_system_prompt_smoke_v5() {
    super::require_nightly_suite();

    let (fixture, ctx) = shared_smoke_v5_context().await;
    let corpus = &fixture.corpus;
    let notebook_id = corpus.notebook_id.clone();
    let doc_ids: Vec<String> = corpus
        .documents
        .iter()
        .map(|doc| doc.document_id.clone())
        .collect();
    let expected_doc_count = std::env::var("RAG_SMOKE_SINGLE_DOC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|_| 1usize)
        .unwrap_or(3);
    assert_eq!(
        doc_ids.len(),
        expected_doc_count,
        "should have {expected_doc_count} documents in corpus"
    );

    // --- Load curated smoke probes (12 examples, 3-doc corpus only) ---
    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rag_quality/golden_set_smoke_v5.json");
    let dataset = GoldenDataset::load(&golden_path).expect("load smoke v5 golden set");
    let smoke_subset = dataset
        .subsets
        .iter()
        .find(|s| s.name == "smoke_v5")
        .unwrap_or_else(|| panic!("smoke v5 golden set missing smoke_v5 subset"));
    let selected: Vec<&GoldenExample> = smoke_subset.examples.iter().collect();
    let selected: Vec<&GoldenExample> = match std::env::var("RAG_SMOKE_V5_QUERIES") {
        Ok(spec) if !spec.trim().is_empty() => {
            let idxs: Vec<usize> = spec
                .split(',')
                .filter_map(|t| t.trim().parse::<usize>().ok())
                .filter(|i| *i >= 1 && *i <= smoke_subset.examples.len())
                .collect();
            eprintln!(
                "[smoke_v5] SUBSET filter RAG_SMOKE_V5_QUERIES={:?} -> {} queries",
                idxs, idxs.len()
            );
            idxs.iter().map(|i| &smoke_subset.examples[*i - 1]).collect()
        }
        _ => selected,
    };
    let is_subset = std::env::var("RAG_SMOKE_V5_QUERIES")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    eprintln!(
        "[smoke_v5] running {} probe queries from {}",
        selected.len(),
        golden_path.display()
    );
    assert!(
        if is_subset { selected.len() >= 1 } else { selected.len() >= 10 },
        "smoke v5 set should have at least {} probes for coverage, got {}",
        if is_subset { 1 } else { 10 },
        selected.len()
    );

    // --- Run evaluation ---
    let doc_scope: Vec<String> = doc_ids.clone();
    let mut recall_results = Vec::new();
    let mut citation_results = Vec::new();
    let mut hallucination_results = Vec::new();
    let mut scorecards = Vec::new();
    let mut smoke_scorecard_rows = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    let mut per_subset_stats: std::collections::HashMap<String, (usize, usize, f64)> =
        std::collections::HashMap::new();

    for (idx, example) in selected.iter().enumerate() {
        let subset_name = smoke_probe_subset_label(example);
        let artifact_key = smoke_probe_artifact_key(idx, example);
        eprintln!(
            "\n[smoke_v5] {}/{} subset={} Q={:?}",
            idx + 1,
            selected.len(),
            subset_name,
            example.query.chars().take(70).collect::<String>()
        );

        let probe =
            match chat_rag_observable_probe(&ctx, &example.query, &notebook_id, &doc_scope).await {
                Ok(p) => p,
                Err(e) => {
                    failures.push((example.query.clone(), format!("chat: {e}")));
                    eprintln!("  FAIL: chat error: {e}");
                    continue;
                }
            };
        let chat = probe.resp;
        let tools = summarize_tool_activity(&probe.sse_events, &chat);
        let turn_count = count_sse_trace_stage(&probe.sse_events, "turn_start");
        let observability_mode = match probe.observability_mode {
            ObservabilityMode::FullStream => "stream",
            ObservabilityMode::FallbackNonStream => "fallback_non_stream",
        };
        let degrade_reasons: Vec<String> = chat
            .degrade_trace
            .iter()
            .map(|d| format!("{:?}", d.reason))
            .collect();
        let disclosed_skills: Vec<String> = probe
            .capture
            .prompt_snapshots
            .iter()
            .filter_map(|snap| {
                snap.get("disclosed_skills")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
            })
            .flatten()
            .collect();
        let artifact_dir = ctx.smoke_v5_probe_artifact_dir(&artifact_key);
        ctx.save_smoke_v5_probe_artifact(
            &artifact_key,
            &chat,
            &probe.capture,
            &probe.sse_events,
            Some(&serde_json::json!({
                "query": example.query,
                "subset": subset_name,
                "probe_index": idx + 1,
                "observability_mode": observability_mode,
                "stream_error_with_done": probe.stream_error_with_done,
                "tools": tools,
                "turn_count": turn_count,
                "disclosed_skills": disclosed_skills,
            })),
        );
        eprintln!(
            "  observability: mode={observability_mode} turns={turn_count} tools={tools:?} \
             skills={disclosed_skills:?} degrade={degrade_reasons:?} \
             trace_reasoning={} prompt_snapshots={} sse_events={} -> {}",
            probe.capture.trace_reasoning.len(),
            probe.capture.prompt_snapshots.len(),
            probe.sse_events.len(),
            artifact_dir.display()
        );
        let retrieved = extract_retrieved_chunks(&chat.tool_results);
        let cited = extract_cited_chunks(&chat.citations);
        let chunks: Vec<String> = retrieved.contents();
        let chunk_to_cite: std::collections::HashMap<String, i64> = chat
            .citations
            .iter()
            .filter_map(|c| c.chunk_id.clone().map(|id| (id, c.citation_id)))
            .collect();
        let answer = rewrite_citations(&chat.answer, &chunk_to_cite);

        let citation_indices = EvaluationMetrics::extract_citation_indices(&answer);
        let recall = EvaluationMetrics::recall_at_k(&example.query, &chunks, example, 15);
        let citation =
            EvaluationMetrics::citation_accuracy(&example.query, &citation_indices, example);
        let halluc = EvaluationMetrics::hallucination_check(&example.query, &answer, &chunks);
        let scorecard = score_query(&retrieved, &cited, &answer, example, 15);

        eprintln!(
            "  recall@15={:.0}% ({}/{}) cit_acc={:.0}% (tp={} missing={:?}) halluc={:.2} chunks={} ans_len={} label={}",
            recall.recall * 100.0,
            recall.matched_chunks.len(),
            recall.golden_count,
            citation.accuracy * 100.0,
            citation.true_positives,
            citation.missing,
            halluc.hallucination_score,
            chunks.len(),
            chat.answer.len(),
            scorecard.label.as_str()
        );
        eprintln!(
            "  expected: {}",
            example
                .expected_answer
                .chars()
                .take(120)
                .collect::<String>()
        );
        eprintln!(
            "  answer_preview: {}",
            chat.answer.chars().take(300).collect::<String>()
        );
        // Judge view (ADR 0011 "in-loop LLM is the judge"): the deterministic
        // layer cannot do semantic faithfulness, so surface the material a human
        // / in-loop LLM needs to judge grounding manually — cited evidence text,
        // the deterministic layer's unsupported-claim flags, and any
        // must_not_include violations. This replaces an in-pipeline LLM-as-Judge
        // for the dev loop.
        for (i, ch) in cited.chunks.iter().take(3).enumerate() {
            eprintln!(
                "  cited[{i}] (id={:?} score={:.2}): {}",
                ch.chunk_id,
                ch.score,
                ch.content.chars().take(140).collect::<String>()
            );
        }
        if !scorecard.faithfulness.unsupported_claims.is_empty() {
            eprintln!(
                "  unsupported_claims (deterministic): {:?}",
                scorecard.faithfulness.unsupported_claims
            );
        }
        let must_not_hits: Vec<&String> = example
            .must_not_include
            .iter()
            .filter(|m| chat.answer.contains(m.as_str()))
            .collect();
        if !must_not_hits.is_empty() {
            eprintln!("  must_not_include VIOLATIONS: {:?}", must_not_hits);
        }
        if example.expected_should_answer && example.must_include.is_empty() {
            eprintln!(
                "  WARN: should_answer but must_include empty — correctness NOT \
                 verified deterministically; requires in-loop review."
            );
        }

        let entry = per_subset_stats
            .entry(subset_name.to_string())
            .or_insert((0, 0, 0.0));
        entry.0 += 1;
        entry.1 += recall.matched_chunks.len();
        entry.2 += recall.recall;

        smoke_scorecard_rows.push(SmokeScorecardRow {
            subset: subset_name.to_string(),
            query: example.query.clone(),
            label: scorecard.label.as_str().to_string(),
            retrieval_recall: scorecard.retrieval.recall,
            selection_precision: scorecard.selection.precision,
            faithfulness: scorecard.faithfulness.faithfulness,
        });
        recall_results.push(recall);
        citation_results.push(citation);
        hallucination_results.push(halluc);
        scorecards.push(scorecard);
    }

    // --- Aggregate and report ---
    let metrics =
        EvaluationMetrics::aggregate(recall_results, citation_results, hallucination_results);
    let scorecard_summary = ScorecardSummary::from_scorecards(&scorecards);

    eprintln!();
    eprintln!("=========================================");
    eprintln!("RAG System Prompt v5 Smoke Report");
    eprintln!("=========================================");
    eprintln!("Prompt version:      rag-system.md v5.0 (agent-centered ReAct)");
    eprintln!("Corpus:              3 documents (thesis + IPD table + Baiyao PDF→TXT)");
    eprintln!("Golden set:          tests/rag_quality/golden_set_smoke_v5.json");
    eprintln!("Probe queries:       {}", metrics.total_examples);
    eprintln!("Recall@15:           {:.2}%", metrics.recall_at_15 * 100.0);
    eprintln!(
        "Citation Accuracy:   {:.2}%",
        metrics.citation_accuracy * 100.0
    );
    eprintln!(
        "Hallucination Rate:  {:.2}%  (heuristic — noise until NLI; not gated)",
        metrics.hallucination_rate * 100.0
    );
    print_scorecard_summary(
        "Decoupled RAG Scorecard (retrieval / selection / generation)",
        &scorecard_summary,
    );
    if let Err(err) = append_smoke_loop_scorecard(&scorecard_summary, &smoke_scorecard_rows) {
        eprintln!("WARN: failed to append smoke scorecard to loop_iterations.md: {err}");
    }

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failures ({}):", failures.len());
        for (q, err) in &failures {
            eprintln!("  - {:?}: {}", q.chars().take(50).collect::<String>(), err);
        }
    }

    eprintln!();
    eprintln!("Per-subset breakdown:");
    eprintln!(
        "  {:<22} {:>6} {:>8} {:>10}",
        "subset", "count", "matched", "avg_recall"
    );
    let mut subset_names: Vec<_> = per_subset_stats.keys().cloned().collect();
    subset_names.sort();
    for name in subset_names {
        if let Some(&(count, matched, sum_recall)) = per_subset_stats.get(&name) {
            eprintln!(
                "  {:<22} {:>6} {:>8} {:>9.1}%",
                name,
                count,
                matched,
                (sum_recall / count as f64) * 100.0
            );
        }
    }

    eprintln!();
    eprintln!("NOTE: smoke probe — no blocking gate. Review per-query output above:");
    eprintln!("  - thesis_*:    factual / synthesis / numeric / adversarial refusal");
    eprintln!("  - ipd_table:   PAC row lookup (PAC-05, PAC-20)");
    eprintln!("  - baiyao_pdf:  4A term + 11/100/638 counts");
    eprintln!("  - cross_doc:   4R vs 4A; IPD 370 vs Baiyao 638");

    assert!(metrics.total_examples > 0, "should have run probe examples");
    assert!(
        failures.len() < selected.len(),
        "all probes failed — check service health. Failures: {failures:?}"
    );
}

/// Tool-coverage probe against `golden_set_tools.json` (8 queries) on the smoke v5
/// 3-document corpus. Reports whether trace tools match `expected_tool` /
/// `expected_tool_sequence` — **not** answer correctness.
///
/// Graph probes (G1/G2) are marked `requires_triplet_reingest: true` and will fail
/// tool coverage until the corpus is re-ingested with `INGESTION_TRIPLET_ENABLED=1`.
///
/// Run with:
/// ```bash
/// E2E_MODE=nightly cargo test -p app --test product_e2e rag_tools_golden_set \
///   --features product-e2e -- --ignored --test-threads=1 --nocapture
/// ```
#[tokio::test]
#[ignore = "requires real LLM + embedding API keys; run with --ignored --test-threads=1"]
async fn rag_tools_golden_set() {
    super::require_nightly_suite();

    let (fixture, ctx) = shared_smoke_v5_context().await;
    let corpus = &fixture.corpus;
    let notebook_id = corpus.notebook_id.clone();
    let doc_ids: Vec<String> = corpus
        .documents
        .iter()
        .map(|doc| doc.document_id.clone())
        .collect();
    assert_eq!(doc_ids.len(), 3, "should have 3 documents in corpus");

    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rag_quality/golden_set_tools.json");
    let dataset = GoldenDataset::load(&golden_path).expect("load tools golden set");
    let tools_subset = dataset
        .subsets
        .iter()
        .find(|s| s.name == "tools_v1")
        .unwrap_or_else(|| panic!("tools golden set missing tools_v1 subset"));
    let selected: Vec<&GoldenExample> = tools_subset.examples.iter().collect();
    eprintln!(
        "[tools_v1] running {} tool-coverage probes from {}",
        selected.len(),
        golden_path.display()
    );

    let doc_scope: Vec<String> = doc_ids.clone();
    let mut tool_scores = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    for (idx, example) in selected.iter().enumerate() {
        let subset_name = smoke_probe_subset_label(example);
        eprintln!(
            "\n[tools_v1] {}/{} subset={} Q={:?}",
            idx + 1,
            selected.len(),
            subset_name,
            example.query.chars().take(70).collect::<String>()
        );
        if example.requires_triplet_reingest {
            eprintln!("  NOTE: requires_triplet_reingest=true (graph probes need triplet re-ingest)");
        }

        let probe =
            match chat_rag_observable_probe(&ctx, &example.query, &notebook_id, &doc_scope).await
            {
                Ok(p) => p,
                Err(e) => {
                    failures.push((example.query.clone(), format!("chat: {e}")));
                    eprintln!("  FAIL: chat error: {e}");
                    continue;
                }
            };
        let chat = probe.resp;
        let sse_tools = summarize_tool_activity(&probe.sse_events, &chat);
        let trace_tools = extract_tool_trace(&chat.tool_results);
        let score = ToolCoverageScore::score(example, &trace_tools);
        tool_scores.push(score.clone());

        eprintln!(
            "  tools(sse)={sse_tools:?} tools(trace)={trace_tools:?} \
             expected={:?} sequence={:?} covered={}",
            example.expected_tool,
            example.expected_tool_sequence,
            score.covered
        );
    }

    let tool_summary = ToolCoverageSummary::from_scores(&tool_scores);
    eprintln!();
    eprintln!("=========================================");
    eprintln!("RAG Tools Golden Set Report (tool coverage only)");
    eprintln!("=========================================");
    eprintln!("Golden set:          tests/rag_quality/golden_set_tools.json");
    eprintln!("Corpus:              3 documents (smoke v5 sub-corpus)");
    print_tool_coverage_summary("Tool Coverage Summary", &tool_summary);

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failures ({}):", failures.len());
        for (q, err) in &failures {
            eprintln!("  - {:?}: {}", q.chars().take(50).collect::<String>(), err);
        }
    }

    eprintln!();
    eprintln!("NOTE: tool-coverage probe — no blocking gate.");
    eprintln!("  - tool_summary/metadata/index: should pass on current corpus");
    eprintln!("  - tool_graph (G1/G2): need INGESTION_TRIPLET_ENABLED=1 re-ingest");

    assert!(tool_summary.with_expectations > 0, "should have tool expectations");
    assert!(
        failures.len() < selected.len(),
        "all probes failed — check service health. Failures: {failures:?}"
    );
}

/// Triplet extraction benchmark: single `huawei_ipd_370_activities.txt` ingest + PAC-05 RAG probe.
///
/// Compare Bailian triplet LLMs (speed via PG ingest duration, quality via graph counts + recall).
///
/// Run via `scripts/benchmark_triplet_models.sh` (sets env per model) or manually:
/// ```bash
/// export TRIPLET_BENCHMARK_MODEL=qwen3.5-flash
/// export RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt
/// export RAG_QUALITY_SMOKE_FORCE_INGEST=1
/// export RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1
/// export TRIPLET_LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
/// export TRIPLET_LLM_API_KEY=$DASHSCOPE_API_KEY
/// export TRIPLET_LLM_MODEL=$TRIPLET_BENCHMARK_MODEL
/// E2E_MODE=nightly cargo test -p app --test product_e2e triplet_benchmark_huawei_ipd \
///   --features product-e2e -- --ignored --test-threads=1 --nocapture
/// ```
///
/// For `qwen-doc-turbo` (256K context), also set `INGESTION_TRIPLET_TOKEN_BUDGET=200000`.
#[tokio::test]
#[ignore = "requires real triplet LLM + embedding API; run via scripts/benchmark_triplet_models.sh"]
async fn triplet_benchmark_huawei_ipd() {
    super::require_nightly_suite();

    let model = std::env::var("TRIPLET_BENCHMARK_MODEL")
        .expect("TRIPLET_BENCHMARK_MODEL must be set (e.g. qwen3.5-flash)");
    let provider = std::env::var("TRIPLET_BENCHMARK_PROVIDER").unwrap_or_else(|_| "unknown".to_string());
    let single_doc = std::env::var("RAG_SMOKE_SINGLE_DOC").unwrap_or_default();
    assert_eq!(
        single_doc.trim(),
        "huawei_ipd_370_activities.txt",
        "benchmark requires RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt"
    );
    let token_budget = std::env::var("INGESTION_TRIPLET_TOKEN_BUDGET")
        .unwrap_or_else(|_| "3000".to_string());

    eprintln!("=========================================");
    eprintln!("Triplet Benchmark: huawei_ipd_370_activities.txt");
    eprintln!("  provider={provider} model={model} token_budget={token_budget}");
    eprintln!("=========================================");

    let (fixture, ctx) = shared_smoke_v5_context().await;
    let corpus = &fixture.corpus;
    assert_eq!(
        corpus.documents.len(),
        1,
        "expected single-doc corpus, got {} docs",
        corpus.documents.len()
    );
    let huawei = corpus
        .documents
        .iter()
        .find(|d| d.filename == "huawei_ipd_370_activities.txt")
        .expect("huawei doc in corpus");
    let doc_id = huawei.document_id.clone();
    let notebook_id = corpus.notebook_id.clone();
    let doc_scope = vec![doc_id.clone()];

    let ingest_secs = ctx
        .query_document_ingest_duration_secs(&doc_id)
        .await
        .expect("ingest duration");
    let chunk_count = ctx
        .query_document_chunk_count(&doc_id)
        .await
        .expect("chunk count");
    let summary = ctx
        .query_latest_backend_summary(&doc_id)
        .await
        .expect("backend_summary");
    let outputs = summary
        .get("outputs")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let entity_count = outputs
        .get("entity_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let relation_count = outputs
        .get("relation_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let graph_passage_count = outputs
        .get("graph_passage_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let graph_degrade_count = outputs
        .get("graph_degrade_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    eprintln!(
        "[benchmark] ingest={ingest_secs:.1}s chunks={chunk_count} \
         entities={entity_count} relations={relation_count} \
         graph_passages={graph_passage_count} graph_degrades={graph_degrade_count}"
    );

    let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rag_quality/golden_set_smoke_v5.json");
    let dataset = GoldenDataset::load(&golden_path).expect("load smoke v5 golden set");
    let example = dataset
        .subsets
        .iter()
        .flat_map(|s| s.examples.iter())
        .find(|e| e.query.contains("PAC-05"))
        .expect("PAC-05 probe in golden set");

    let probe =
        chat_rag_observable_probe(&ctx, &example.query, &notebook_id, &doc_scope)
            .await
            .expect("PAC-05 RAG probe");
    let chat = probe.resp;
    let retrieved = extract_retrieved_chunks(&chat.tool_results);
    let cited = extract_cited_chunks(&chat.citations);
    let chunks: Vec<String> = retrieved.contents();
    let chunk_to_cite: std::collections::HashMap<String, i64> = chat
        .citations
        .iter()
        .filter_map(|c| c.chunk_id.clone().map(|id| (id, c.citation_id)))
        .collect();
    let answer = rewrite_citations(&chat.answer, &chunk_to_cite);
    let recall = EvaluationMetrics::recall_at_k(&example.query, &chunks, example, 15);
    let scorecard = score_query(&retrieved, &cited, &answer, example, 15);

    eprintln!(
        "[benchmark] PAC-05 recall@15={:.0}% label={} faithfulness={:.0}%",
        recall.recall * 100.0,
        scorecard.label.as_str(),
        scorecard.faithfulness.faithfulness * 100.0
    );
    eprintln!("[benchmark] answer: {}", answer.chars().take(200).collect::<String>());

    let result = serde_json::json!({
        "provider": provider,
        "model": model,
        "token_budget": token_budget.parse::<i64>().unwrap_or(3000),
        "ingest_secs": ingest_secs,
        "chunk_count": chunk_count,
        "entity_count": entity_count,
        "relation_count": relation_count,
        "graph_passage_count": graph_passage_count,
        "graph_degrade_count": graph_degrade_count,
        "recall_at_15": recall.recall,
        "diagnostic_label": scorecard.label.as_str(),
        "faithfulness": scorecard.faithfulness.faithfulness,
        "answer_preview": answer.chars().take(300).collect::<String>(),
    });
    eprintln!("TRIPLET_BENCHMARK_RESULT={}", result);

    assert!(
        graph_passage_count > 0 || graph_degrade_count > 0,
        "triplet pipeline produced no graph output — check TRIPLET_LLM_* / INGESTION_TRIPLET_ENABLED"
    );
}
