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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use rag_quality::{
    CitationAccuracyResult, EvaluationMetrics, GoldenDataset, GoldenExample, HallucinationResult,
    PerQueryScorecard, RecallResult, ScorecardSummary, ToolCoverageScore, ToolCoverageSummary,
    extract_cited_chunks, extract_retrieved_chunks, extract_tool_trace, score_query,
};
// ADR-0012 eval v2 (judge-first generation metrics). Phase 0: report-only.
use rag_quality::circuit_breaker::ConsecutiveNonPassBreaker;
use rag_quality::eval_v2;
use regex::Regex;

use super::{
    ObservabilityMode, chat_rag_observable_probe, count_sse_trace_stage, probe_api_liveness,
    summarize_tool_activity,
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

/// Retrieval cutoff used by this runner (recall@15 / score_query k=15).
const RETRIEVAL_K: usize = 15;

/// Max chars of a tool result's JSON payload rendered into one judge-context
/// block (keeps a runaway payload from dominating the prompt).
const TOOL_OUTPUT_MAX_CHARS: usize = 2000;

/// Render builtin (non-retrieval) tool outputs as judge-context blocks for
/// non-RAG turns (q125 class): every Ok tool result whose tool is NOT in the
/// retrieval set, as `"tool: <json payload (trimmed)>"`. Retrieval chunks
/// already flow through `retrieved`/`cited`; this is the weather_query /
/// calculator / doc_profile / user_context evidence that otherwise never
/// reached the judge.
fn builtin_tool_outputs(tool_results: &[contracts::ToolResult]) -> Vec<String> {
    tool_results
        .iter()
        .filter(|r| r.status == contracts::ToolStatus::Ok)
        .filter(|r| !rag_quality::harness_extract::RETRIEVAL_TOOLS.contains(&r.tool.as_str()))
        .filter_map(|r| {
            let data = r.data.as_ref()?;
            let payload = serde_json::to_string(data).ok()?;
            let payload: String = payload.chars().take(TOOL_OUTPUT_MAX_CHARS).collect();
            Some(format!("{}: {}", r.tool, payload))
        })
        .collect()
}

/// True when the env var is set to `1` or `true` (case-insensitive); absent or
/// any other value means off.
fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// True only on an explicit opt-out (`0` or `false`, case-insensitive);
/// absent or any other value means NOT disabled.
fn env_flag_disabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
}

/// True when the answer carries code spans (`<code>…</code>` or markdown
/// fences) but no prose outside them — the retrieve-phase "output one code
/// block" framing leaked into the final answer. Infrastructure/contract
/// failure, not a retrieval miss. Mirror of the agent-loop prose_only
/// detector; duplicated here to keep the harness dependency-free.
fn is_code_only_answer(answer: &str) -> bool {
    let mut saw_code = false;
    let mut outside = String::new();
    let mut rest = answer;
    while let Some(start) = rest.find("<code") {
        let Some(tag_end) = rest[start..].find('>').map(|o| start + o) else {
            break;
        };
        let Some(close) = rest[tag_end..].find("</code>").map(|o| tag_end + o) else {
            break;
        };
        outside.push_str(&rest[..start]);
        saw_code = true;
        rest = &rest[close + "</code>".len()..];
    }
    outside.push_str(rest);
    let mut prose = String::new();
    let mut in_fence = false;
    for line in outside.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            saw_code = true;
            continue;
        }
        if !in_fence {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    saw_code && prose.trim().is_empty()
}

/// Dynamic-QC loop observability mirrored by app-chat into
/// `ChatResponse.mode_debug.general` (see `insert_loop_observability` in
/// app-chat `chat/pipeline_steps.rs`). All fields optional so artifacts stay
/// comparable with runs predating the mirror.
#[derive(Debug, Clone, Default)]
struct LoopObservability {
    /// Terminal exit reason of the agent loop (last per-round decision, e.g.
    /// `evidence_missing_continue` on an early round or the stop reason).
    exit_reason: Option<String>,
    /// Pre-loop query-card (question_type + required_actions), when the L0
    /// classification call produced one.
    query_card: Option<QueryCardObservability>,
    /// Compact rounds summary: iteration count, tool calls, per-round exits.
    loop_rounds: Option<LoopRoundsObservability>,
}

/// Typed view of the query-card mirrored into `mode_debug.general`
/// (shape: `agent_loop::react_loop::query_card::QueryCard`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct QueryCardObservability {
    question_type: String,
    #[serde(default)]
    required_actions: Vec<String>,
}

/// Typed view of the compact rounds summary mirrored into
/// `mode_debug.general.loop_rounds`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LoopRoundsObservability {
    iterations: usize,
    total_tool_calls: u32,
    #[serde(default)]
    exit_reasons: Vec<String>,
}

fn extract_loop_observability(chat: &ChatResponse) -> LoopObservability {
    let Some(general) = chat
        .mode_debug
        .as_ref()
        .and_then(|d| d.general.as_ref())
    else {
        return LoopObservability::default();
    };
    LoopObservability {
        exit_reason: general
            .get("exit_reason")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        query_card: general
            .get("query_card")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        loop_rounds: general
            .get("loop_rounds")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
    }
}

/// Truncate error/stderr strings for eval dumps (P1-b). Char-safe on UTF-8.
const TOOL_TRACE_ERR_CAP: usize = 500;

fn truncate_utf8(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Compact one tool_result for qNNN.json + v2 artifact (request + vgrag + error/stderr).
fn compact_tool_trace_entry(r: &contracts::ToolResult) -> serde_json::Value {
    let data = r.data.as_ref();
    let request = data.and_then(|d| {
        d.get("request_query")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                d.get("request_terms")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
            })
            .or_else(|| {
                d.get("request_pattern")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
    });
    let vgrag_graph_n = data.and_then(|d| d.get("vgrag_graph_n").and_then(|v| v.as_u64()));
    let vgrag_relation_n = data.and_then(|d| d.get("vgrag_relation_n").and_then(|v| v.as_u64()));
    let vgrag_evidence_raw_n =
        data.and_then(|d| d.get("vgrag_evidence_raw_n").and_then(|v| v.as_u64()));
    let vgrag_evidence_dropped =
        data.and_then(|d| d.get("vgrag_evidence_dropped").and_then(|v| v.as_u64()));
    let retrieval_path = data
        .and_then(|d| d.get("retrieval_path").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    // Prefer structured error fields; fall back to degrade_reason.
    let error = data
        .and_then(|d| {
            d.get("error").and_then(|e| {
                if let Some(s) = e.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = e.as_object() {
                    obj.get("message")
                        .and_then(|m| m.as_str())
                        .or_else(|| obj.get("code").and_then(|c| c.as_str()))
                        .map(|s| s.to_string())
                        .or_else(|| Some(e.to_string()))
                } else {
                    Some(e.to_string())
                }
            })
        })
        .or_else(|| {
            data.and_then(|d| d.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
        })
        .or_else(|| {
            r.trace
                .as_ref()
                .and_then(|t| t.degrade_reason.clone())
        })
        .map(|s| truncate_utf8(&s, TOOL_TRACE_ERR_CAP));

    let stderr = data
        .and_then(|d| d.get("stderr").and_then(|v| v.as_str()))
        .map(|s| truncate_utf8(s, TOOL_TRACE_ERR_CAP));

    serde_json::json!({
        "tool": r.tool,
        "status": format!("{:?}", r.status),
        "request": request,
        "vgrag_graph_n": vgrag_graph_n,
        "vgrag_relation_n": vgrag_relation_n,
        "vgrag_evidence_raw_n": vgrag_evidence_raw_n,
        "vgrag_evidence_dropped": vgrag_evidence_dropped,
        "retrieval_path": retrieval_path,
        "error": error,
        "stderr": stderr,
    })
}

fn compact_tool_trace(results: &[contracts::ToolResult]) -> Vec<serde_json::Value> {
    results.iter().map(compact_tool_trace_entry).collect()
}

#[cfg(test)]
mod loop_observability_tests {
    use super::*;

    fn chat_with_general(general: std::collections::BTreeMap<String, serde_json::Value>) -> ChatResponse {
        ChatResponse {
            answer: "a".to_string(),
            answer_blocks: Vec::new(),
            session_id: "s".to_string(),
            agent_type: "rag".to_string(),
            sources: Vec::new(),
            citations: Vec::new(),
            trace: contracts::chat::TraceInfo {
                mode: "rag".to_string(),
            },
            degrade_trace: Vec::new(),
            planner_output: None,
            mode_debug: Some(contracts::chat::ModeDebug {
                rag: None,
                search: None,
                general: Some(general),
            }),
            message_id: None,
            guard_report: None,
            tool_results: Vec::new(),
            usage: None,
            agent_operation_guide: None,
        }
    }

    #[test]
    fn extracts_exit_reason_query_card_and_rounds() {
        let general = serde_json::json!({
            "exit_reason": "evidence_missing_continue",
            "query_card": { "question_type": "rag_fact", "required_actions": ["dense"] },
            "loop_rounds": {
                "iterations": 2,
                "total_tool_calls": 3,
                "exit_reasons": ["evidence_missing_continue", "code_gen"],
            },
        });
        let chat = chat_with_general(
            general
                .as_object()
                .expect("object")
                .clone()
                .into_iter()
                .collect(),
        );
        let obs = extract_loop_observability(&chat);
        assert_eq!(obs.exit_reason.as_deref(), Some("evidence_missing_continue"));
        let card = obs.query_card.expect("query_card parsed");
        assert_eq!(card.question_type, "rag_fact");
        assert_eq!(card.required_actions, vec!["dense".to_string()]);
        let rounds = obs.loop_rounds.expect("loop_rounds parsed");
        assert_eq!(rounds.iterations, 2);
        assert_eq!(rounds.total_tool_calls, 3);
        assert_eq!(
            rounds.exit_reasons,
            vec![
                "evidence_missing_continue".to_string(),
                "code_gen".to_string()
            ]
        );
    }

    #[test]
    fn absent_keys_yield_none_fields() {
        let mut chat = chat_with_general(std::collections::BTreeMap::new());
        let obs = extract_loop_observability(&chat);
        assert!(obs.exit_reason.is_none());
        assert!(obs.query_card.is_none());
        assert!(obs.loop_rounds.is_none());
        // No mode_debug at all (older server builds) → same absence.
        chat.mode_debug = None;
        let obs = extract_loop_observability(&chat);
        assert!(obs.exit_reason.is_none());
        assert!(obs.query_card.is_none());
        assert!(obs.loop_rounds.is_none());
    }

    #[test]
    fn truncate_utf8_is_char_safe() {
        let s = "测".repeat(10);
        let t = truncate_utf8(&s, 3);
        assert_eq!(t.chars().count(), 4); // 3 chars + ellipsis
        assert!(t.ends_with('…'));
    }

    #[test]
    fn compact_tool_trace_captures_request_vgrag_and_error() {
        let long_err = "x".repeat(600);
        let r = contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({
                "request_query": "实体甲",
                "vgrag_graph_n": 3,
                "vgrag_relation_n": 5,
                "vgrag_evidence_raw_n": 5,
                "vgrag_evidence_dropped": 2,
                "retrieval_path": "vgrag",
                "error": long_err,
                "stderr": "TypeError: boom",
            })),
            trace: None,
        };
        let v = compact_tool_trace_entry(&r);
        assert_eq!(v["tool"], "dense_retrieval");
        assert_eq!(v["request"], "实体甲");
        assert_eq!(v["vgrag_graph_n"], 3);
        assert_eq!(v["retrieval_path"], "vgrag");
        let err = v["error"].as_str().expect("error string");
        assert!(err.chars().count() <= TOOL_TRACE_ERR_CAP + 1);
        assert!(err.ends_with('…'));
        assert_eq!(v["stderr"], "TypeError: boom");
    }
}

/// ADR-0012 eval-v2 run state: one judge client, one per-run artifact dir
/// (`tests/e2e_output/rag_eval_v2/{run_id}/`), the judge response cache
/// (design §4.4, shared across run ids), and the per-question `ScoreV2` sink
/// aggregated at end of run (design §9 runner integration). Phase 0:
/// everything here is report-only — no assertion reads these numbers.
struct V2RunCtx {
    judge: eval_v2::JudgeClient,
    run_id: String,
    run_dir: std::path::PathBuf,
    cache: eval_v2::JudgeCache,
    /// Concurrent-sink for per-question `ScoreV2`s: the eval loop runs
    /// questions in parallel (buffer_unordered), so this is an
    /// `Arc<Mutex<Vec<_>>>`; aggregated serially at end of run.
    scores: std::sync::Arc<std::sync::Mutex<Vec<eval_v2::ScoreV2>>>,
    /// `E2E_ABORT_AFTER_CONSECUTIVE_FAILS` breaker (default 8, 0 disables):
    /// trips on a trailing run of consecutive non-PASS v2 labels so a systemic
    /// break stops scheduling new questions instead of burning the full run.
    breaker: std::sync::Arc<std::sync::Mutex<ConsecutiveNonPassBreaker>>,
}

/// Outcome of the cached judge call path for one question.
struct JudgeAttempt {
    status: eval_v2::JudgeStatus,
    parsed: Option<eval_v2::JudgeOutput>,
    /// Raw response of the last live attempt (or the cached raw on a hit);
    /// kept even on parse failure for debugging.
    raw: Option<String>,
    note: String,
    cache_hit: bool,
}

impl V2RunCtx {
    /// Feed one question's final v2 label to the circuit breaker; logs once
    /// on the record that trips it.
    fn note_label(&self, qnum: usize, label: eval_v2::LabelV2) {
        let newly_tripped = self
            .breaker
            .lock()
            .expect("breaker mutex")
            .record(qnum, label == eval_v2::LabelV2::Pass);
        if newly_tripped {
            eprintln!(
                "  [circuit-breaker] E2E_ABORT_AFTER_CONSECUTIVE_FAILS trip at q{qnum}: \
                 {} consecutive non-PASS — remaining questions skip",
                self.breaker.lock().expect("breaker mutex").threshold()
            );
        }
    }

    /// Whether the circuit breaker has tripped (unstarted questions should
    /// skip, mirroring the fail-fast early-out).
    fn abort_requested(&self) -> bool {
        self.breaker.lock().expect("breaker mutex").tripped()
    }

    /// Record an infrastructure failure (chat transport / HTTP 5xx / error
    /// envelope / parse error / empty answer): Layer A scores over empty
    /// chunks, label INFRA_ERROR, judge call skipped — there is no answer to
    /// judge (design §5 priority 0).
    fn record_infra(&self, qnum: usize, example: &GoldenExample, subset: &str, reason: &str) {
        let empty_retrieved = rag_quality::RetrievedChunks::default();
        let empty_cited = rag_quality::CitedChunks::default();
        let context_source =
            eval_v2::ContextSource::determine(example, &empty_retrieved, &empty_cited);
        let retrieval = eval_v2::score_retrieval(&empty_retrieved, example, RETRIEVAL_K);
        let selection = eval_v2::score_selection(&empty_cited, example);
        let score = self.finish_score(
            example,
            subset,
            retrieval,
            selection,
            eval_v2::JudgeStatus::Error,
            None,
            true,
            None,
            context_source,
        );
        self.write_json(
            &format!("q{qnum:03}.artifact.json"),
            &serde_json::json!({
                "question": example.query,
                "subset": subset,
                "infra_error": reason,
                "answer": serde_json::Value::Null,
                "context_source": context_source.as_str(),
                "score_v2": score,
            }),
        );
        self.note_label(qnum, score.label);
        self.scores.lock().expect("scores mutex").push(score);
    }

    /// Layer A + Layer B for one answered question (design §9): deterministic
    /// retrieval/selection scores, then one serial Flash judge call. Broken
    /// JSON gets exactly one retry (design §4.3); transport errors and second
    /// parse failures map to JUDGE_ERROR and the loop continues.
    async fn score_question(
        &self,
        qnum: usize,
        example: &GoldenExample,
        subset: &str,
        retrieved: &rag_quality::RetrievedChunks,
        cited: &rag_quality::CitedChunks,
        answer: &str,
        tool_outputs: &[String],
        obs: &LoopObservability,
        tool_trace: &[serde_json::Value],
    ) {
        let retrieval = eval_v2::score_retrieval(retrieved, example, RETRIEVAL_K);
        let selection = eval_v2::score_selection(cited, example);
        let judge_input = eval_v2::JudgeInput::new(example, retrieved, cited, answer, tool_outputs);
        let messages = vec![
            avrag_llm::ChatMessage::system(eval_v2::SYSTEM_PROMPT),
            avrag_llm::ChatMessage::user(eval_v2::build_user_prompt(&judge_input)),
        ];
        let attempt = self.judge_with_retry(&messages, &judge_input).await;
        let score = self.finish_score(
            example,
            subset,
            retrieval,
            selection,
            attempt.status,
            attempt.parsed,
            false,
            Some(answer),
            judge_input.context_source,
        );
        // The judge's raw refusal boolean is advisory; flag when it disagrees
        // with the derived value (the q009-class judge mislabel).
        let refusal_raw_mismatch = score.judge.as_ref().is_some_and(|j| {
            eval_v2::derived_refusal_correct(&j.refusal, example.expected_should_answer)
                != j.refusal.correct_for_expectation
        });
        // Full-stream recall is primary; show the top-k view only when it
        // diverges (multi-round surfacing: gold found after rank 15).
        let recall_str = if (score.retrieval.recall - score.retrieval.recall_at_k).abs() < 1e-9 {
            format!("{:.2}", score.retrieval.recall)
        } else {
            format!(
                "{:.2} (@{}={:.2})",
                score.retrieval.recall, score.retrieval.k, score.retrieval.recall_at_k
            )
        };
        eprintln!(
            "  v2: label={} recall={} judge={:?} cache={} correctness={} faithfulness={}{}",
            score.label.as_str(),
            recall_str,
            attempt.status,
            if attempt.cache_hit { "hit" } else { "miss" },
            score
                .judge
                .as_ref()
                .map(|j| j.answer_correctness.score.to_string())
                .unwrap_or_else(|| "-".to_string()),
            score
                .judge
                .as_ref()
                .map(|j| j.faithfulness.score.to_string())
                .unwrap_or_else(|| "-".to_string()),
            if refusal_raw_mismatch {
                " refusal_raw_mismatch"
            } else {
                ""
            },
        );
        // Judge artifact: raw response kept even on parse failure (debugging).
        self.write_json(
            &format!("q{qnum:03}.judge.json"),
            &serde_json::json!({
                "question": example.query,
                "subset": subset,
                "schema_version": eval_v2::SCHEMA_VERSION,
                "judge_model": self.judge.model(),
                "judge_status": attempt.status,
                "cache": if attempt.cache_hit { "hit" } else { "miss" },
                "note": attempt.note,
                "raw_response": attempt.raw,
                "parsed": score.judge,
            }),
        );
        // Dynamic-QC observability (exit_reason / query_card / loop_rounds) is
        // optional — keys are skipped when the run predates the mode_debug
        // mirror so old artifacts stay comparable.
        let mut artifact = serde_json::json!({
            "question": example.query,
            "subset": subset,
            "answer": answer,
            "context_source": judge_input.context_source.as_str(),
            // M1(2026-08-01):judge 输入快照持久化——JUDGE_ERROR 题可离线
            // 重判(rejudge 入口读此重建 JudgeInput),无需重生成答案。
            "judge_input": judge_input,
            "score_v2": score,
        });
        if let Some(exit_reason) = obs.exit_reason.as_deref() {
            artifact["exit_reason"] = serde_json::json!(exit_reason);
        }
        if let Some(query_card) = obs.query_card.as_ref() {
            artifact["query_card"] = serde_json::json!(query_card);
        }
        if let Some(loop_rounds) = obs.loop_rounds.as_ref() {
            artifact["loop_rounds"] = serde_json::json!(loop_rounds);
        }
        // P1-c: same compact tool_trace as realistic_corpus_full_eval/qNNN.json
        // so behavior diagnosis does not need a second directory.
        if !tool_trace.is_empty() {
            artifact["tool_trace"] = serde_json::json!(tool_trace);
        }
        self.write_json(&format!("q{qnum:03}.artifact.json"), &artifact);
        self.note_label(qnum, score.label);
        self.scores.lock().expect("scores mutex").push(score);
    }

    /// Assemble a `ScoreV2` from its parts, deriving the label via the
    /// score-driven priority table (design §5) with the initial thresholds.
    #[allow(clippy::too_many_arguments)]
    fn finish_score(
        &self,
        example: &GoldenExample,
        subset: &str,
        retrieval: eval_v2::RetrievalScoreV2,
        selection: eval_v2::SelectionScoreV2,
        judge_status: eval_v2::JudgeStatus,
        judge: Option<eval_v2::JudgeOutput>,
        has_infra_error: bool,
        answer: Option<&str>,
        context_source: eval_v2::ContextSource,
    ) -> eval_v2::ScoreV2 {
        let label = eval_v2::label_for(&eval_v2::LabelInput {
            has_infra_error,
            judge_status,
            gold_exists: !example.source_chunks.is_empty(),
            no_context: context_source == eval_v2::ContextSource::NoContext,
            expect_no_retrieval: example.expect_no_retrieval,
            expected_should_answer: example.expected_should_answer,
            retrieval_recall: retrieval.recall,
            cited_gold_hits: selection.golden_matched_in_cited,
            judge: judge.as_ref(),
            thresholds: &eval_v2::JudgeThresholds::default(),
        });
        eval_v2::ScoreV2 {
            query: example.query.clone(),
            subset: subset.to_string(),
            retrieval,
            selection,
            judge,
            judge_status,
            label,
            reference_answer: Some(example.reference_answer().to_string()),
            model_answer: answer.map(str::to_string),
            context_source,
            expect_no_retrieval: example.expect_no_retrieval,
        }
    }

    /// Judge call for one question with the design §4.4 cache in front:
    /// verified cache hit → parse the cached raw (no retry needed); miss →
    /// `live_judge_call`. A cached raw that no longer parses (corrupt or
    /// hand-edited file) falls through to a live call and is overwritten.
    async fn judge_with_retry(
        &self,
        messages: &[avrag_llm::ChatMessage],
        input: &eval_v2::JudgeInput,
    ) -> JudgeAttempt {
        let key = eval_v2::JudgeCache::key(self.judge.model(), input);
        if let Some(raw) = self.cache.load(&key, self.judge.model(), input) {
            if let Ok(parsed) = eval_v2::parse_judge_output(&raw) {
                return JudgeAttempt {
                    status: eval_v2::JudgeStatus::Ok,
                    parsed: Some(parsed),
                    raw: Some(raw),
                    note: "cache_hit".to_string(),
                    cache_hit: true,
                };
            }
        }
        self.live_judge_call(messages, &key, input).await
    }

    /// Live judge call with backoff retry: transport errors retry up to 3
    /// times with 1s/3s backoff; JSON parse failures get one retry (design
    /// §4.3). Successful raw responses are stored in the cache; errors are
    /// never cached.
    async fn live_judge_call(
        &self,
        messages: &[avrag_llm::ChatMessage],
        cache_key: &str,
        input: &eval_v2::JudgeInput,
    ) -> JudgeAttempt {
        // Transport errors: exponential backoff (1s/3s), up to 3 attempts
        // (2026-08-01: judge API transient failures ~7% per run).
        let resp = {
            let mut last_err = None;
            let mut resp = None;
            for attempt in 0..3 {
                match self.judge.complete(messages).await {
                    Ok(r) => {
                        resp = Some(r);
                        break;
                    }
                    Err(e) if attempt < 2 => {
                        let wait = 1u64 << attempt; // 1s, 2s
                        eprintln!(
                            "  v2: judge transport error ({e}); retry {}/3 after {wait}s",
                            attempt + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                        last_err = Some(e);
                    }
                    Err(e) => {
                        last_err = Some(e);
                        break;
                    }
                }
            }
            match resp {
                Some(r) => r,
                None => {
                    return JudgeAttempt {
                        status: eval_v2::JudgeStatus::Error,
                        parsed: None,
                        raw: None,
                        note: format!(
                            "judge transport error after 3 attempts: {:?}",
                            last_err.as_ref().map(|e| e.to_string())
                        ),
                        cache_hit: false,
                    };
                }
            }
        };
        let raw = resp.content;
        let first_err = match eval_v2::parse_judge_output(&raw) {
            Ok(parsed) => {
                self.cache.store(cache_key, self.judge.model(), input, &raw);
                return JudgeAttempt {
                    status: eval_v2::JudgeStatus::Ok,
                    parsed: Some(parsed),
                    raw: Some(raw),
                    note: "ok".to_string(),
                    cache_hit: false,
                };
            }
            Err(e) => e,
        };
        eprintln!("  v2: judge JSON parse failed ({first_err}); retrying once");
        let resp2 = match self.judge.complete(messages).await {
            Ok(resp) => resp,
            Err(e) => {
                return JudgeAttempt {
                    status: eval_v2::JudgeStatus::Error,
                    parsed: None,
                    raw: Some(raw),
                    note: format!("judge retry transport error (first parse: {first_err}): {e}"),
                    cache_hit: false,
                };
            }
        };
        let raw2 = resp2.content;
        match eval_v2::parse_judge_output(&raw2) {
            Ok(parsed) => {
                self.cache
                    .store(cache_key, self.judge.model(), input, &raw2);
                JudgeAttempt {
                    status: eval_v2::JudgeStatus::Ok,
                    parsed: Some(parsed),
                    raw: Some(raw2),
                    note: "ok_after_retry".to_string(),
                    cache_hit: false,
                }
            }
            Err(e) => JudgeAttempt {
                status: eval_v2::JudgeStatus::Error,
                parsed: None,
                raw: Some(raw2),
                note: format!("judge JSON invalid after one retry: {e}"),
                cache_hit: false,
            },
        }
    }

    /// Write summary.json + summary.md + per_query.tsv + judge_prompt_version
    /// (design §7.1) and print the compact Phase-0 report block (report-only —
    /// nothing here gates the test; design §7.2 Phase 0).
    fn print_and_write_summary(&self) {
        let scores = self
            .scores
            .lock()
            .expect("scores mutex")
            .clone();
        let summary = eval_v2::SuiteSummaryV2::from_scores(&scores);
        self.write_json(
            "summary.json",
            &serde_json::json!({
                "judge_model": self.judge.model(),
                "schema_version": eval_v2::SCHEMA_VERSION,
                "thresholds": eval_v2::JudgeThresholds::default(),
                "summary": summary,
            }),
        );
        self.write_text(
            "summary.md",
            &eval_v2::render_summary_md(
                &self.run_id,
                self.judge.model(),
                &eval_v2::JudgeThresholds::default(),
                &scores,
                &summary,
            ),
        );
        self.write_text(
            "per_query.tsv",
            &eval_v2::render_per_query_tsv(&scores),
        );
        // Prompt version marker (design §7.1): schema version + git short hash
        // when the CI env provides one (same option_env convention as the
        // TestContext artifact ids; "local" otherwise).
        let short_commit = option_env!("GITHUB_SHA")
            .map(|s| &s[..s.len().min(8)])
            .unwrap_or("local");
        self.write_text(
            "judge_prompt_version",
            &format!("{} git={short_commit}\n", eval_v2::SCHEMA_VERSION),
        );
        eprintln!();
        eprintln!("RAG Eval v2 (ADR-0012 judge-first) — REPORT ONLY, no gate");
        eprintln!("  judge_model: {}", self.judge.model());
        eprintln!(
            "  judge calls: ok={} error={} (JUDGE_ERROR must be 0 before any gate)",
            summary.judge_ok, summary.judge_error
        );
        eprintln!(
            "  mean answer_correctness={:.3} faithfulness={:.3} relevancy={:.3} (judge-ok; faithfulness n={} excl. no-context/NA)",
            summary.mean_answer_correctness,
            summary.mean_faithfulness,
            summary.mean_answer_relevancy,
            summary.faithfulness_applicable
        );
        eprintln!(
            "  mean retrieval recall={:.2}% (@{}={:.2}%, n={} excl. expect_no_retrieval)",
            summary.mean_retrieval_recall * 100.0,
            RETRIEVAL_K,
            summary.mean_retrieval_recall_at_k * 100.0,
            summary.retrieval_applicable
        );
        let labels = summary
            .label_counts
            .iter()
            .map(|(label, count)| format!("{}={count}", label.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("  labels: {labels}");
        eprintln!("  artifacts: {}", self.run_dir.display());
    }

    fn write_json(&self, filename: &str, value: &serde_json::Value) {
        if let Ok(json) = serde_json::to_string_pretty(value) {
            let _ = std::fs::write(self.run_dir.join(filename), json);
        }
    }

    fn write_text(&self, filename: &str, contents: &str) {
        let _ = std::fs::write(self.run_dir.join(filename), contents);
    }
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

    let workspace_id = &upload.workspace_id;
    let doc_scope = [upload.document_id.clone()];

    let mut recall_results = Vec::new();
    let mut citation_results = Vec::new();
    let mut hallucination_results = Vec::new();
    let mut scorecards = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    for example in &examples {
        let resp = match ctx.chat(&example.query, workspace_id, &doc_scope).await {
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
/// v3: map `doc_scope_hint` to concrete doc ids (orchestrator golden set).
/// `"all"`/unknown → full corpus; `"empty"` → no docs (empty-selection rule);
/// corpus keys restrict scope (cross-doc isolation probes).
fn resolve_doc_scope(hint: &str, scope_keys: &[&str], doc_ids: &[String]) -> Vec<String> {
    let pick = |key: &str| {
        scope_keys
            .iter()
            .position(|k| *k == key)
            .map(|i| doc_ids[i].clone())
    };
    match hint {
        "empty" => Vec::new(),
        "thesis" => pick("thesis").into_iter().collect(),
        "adr_pair" => ["adr4", "adr9"].iter().filter_map(|k| pick(k)).collect(),
        "consulting_platform" => pick("consulting_platform").into_iter().collect(),
        "consulting_compensation" => pick("consulting_compensation").into_iter().collect(),
        "ipd" => pick("ipd").into_iter().collect(),
        "baiyao" => pick("baiyao").into_iter().collect(),
        "rbf" => pick("rbf").into_iter().collect(),
        "prepared_food" => pick("prepared_food").into_iter().collect(),
        "craftsman" => pick("craftsman").into_iter().collect(),
        _ => doc_ids.to_vec(),
    }
}

/// Per-question result from a parallel `run_single_question` task. The metrics
/// fields are `Option` because failure branches (transport / 5xx / envelope /
/// parse / empty / code-only) skip the success-path scoring entirely — the
/// main loop aggregates by `idx`, so the report order stays deterministic.
struct QuestionOutcome {
    idx: usize,
    failures: Vec<(String, String)>,
    subset: String,
    recall: Option<RecallResult>,
    citation: Option<CitationAccuracyResult>,
    halluc: Option<HallucinationResult>,
    scorecard: Option<PerQueryScorecard>,
}

/// One golden question end-to-end: chat → failure classification → retrieval /
/// citation extraction → qNNN snapshot → G-16 / citations / G-17 gates → v2
/// judge → legacy metrics. Returns a self-contained outcome so the main loop
/// can run many questions concurrently (`buffer_unordered`). `v2` shares its
/// `scores` through `Arc<Mutex<_>>`; `fail_fast_flag` is set on the first
/// failure and checked at the top so unstarted tasks skip the rest.
#[allow(clippy::too_many_arguments)]
async fn run_single_question(
    idx: usize,
    example: &GoldenExample,
    dataset: &GoldenDataset,
    scope_keys: &[&str],
    doc_ids: &[String],
    workspace_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    v2: Option<&V2RunCtx>,
    fail_fast: bool,
    fail_fast_flag: &AtomicBool,
    examples_len: usize,
) -> QuestionOutcome {
    let subset_name = dataset
        .subsets
        .iter()
        .find(|s| s.examples.iter().any(|e| e.query == example.query))
        .map(|s| s.name.as_str())
        .unwrap_or("unknown");
    eprintln!(
        "\n[realistic_corpus] {}/{} subset={} Q={:?}",
        idx + 1,
        examples_len,
        subset_name,
        example.query.chars().take(60).collect::<String>()
    );
    if fail_fast_flag.load(Ordering::Relaxed)
        || v2.is_some_and(V2RunCtx::abort_requested)
    {
        return QuestionOutcome {
            idx,
            failures: Vec::new(),
            subset: subset_name.to_string(),
            recall: None,
            citation: None,
            halluc: None,
            scorecard: None,
        };
    }
    let mut outcome = QuestionOutcome {
        idx,
        failures: Vec::new(),
        subset: subset_name.to_string(),
        recall: None,
        citation: None,
        halluc: None,
        scorecard: None,
    };
    let mut mark_failure = |outcome: &mut QuestionOutcome, msg: String| {
        outcome.failures.push((example.query.clone(), msg));
        if fail_fast {
            fail_fast_flag.store(true, Ordering::Relaxed);
        }
    };
    // Non-fatal check helpers return whether to continue with scoring (false =
    // skip, matching the original loop's `continue` on hard failures).
    let scope = resolve_doc_scope(&example.doc_scope_hint, scope_keys, doc_ids);
    let caps = example.resolved_capabilities();
    let turns: Vec<(String, String)> = example
        .prior_turns
        .iter()
        .map(|t| (t.query.clone(), t.answer.clone()))
        .collect();
    let resp = match crate::product_e2e::test_context::post_rag_chat(
        client,
        base_url,
        &example.query,
        workspace_id,
        &scope,
        Some(&caps),
        if turns.is_empty() { None } else { Some(&turns) },
        example.client_time.as_deref(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  FAIL: chat error: {e}");
            mark_failure(&mut outcome, format!("chat: {e}"));
            if let Some(v2) = v2 {
                v2.record_infra(idx + 1, example, subset_name, "chat_transport");
            }
            return outcome;
        }
    };
    let resp_status = resp.status;
    let resp_body = resp.body_json.clone();
    if resp_status >= 500 {
        let raw = serde_json::to_string_pretty(&resp_body)
            .unwrap_or_else(|_| "<serialize failed>".to_string());
        let msg = format!(
            "http_error status={} (internal_error envelope; ignore agent_operation_guide if present)",
            resp_status
        );
        mark_failure(&mut outcome, msg.clone());
        eprintln!("  FAIL: {msg}");
        let preview: String = raw.chars().take(4000).collect();
        eprintln!("  raw response: {preview}");
        if let Some(v2) = v2 {
            v2.record_infra(idx + 1, example, subset_name, "http_5xx");
        }
        return outcome;
    }
    if resp_body.get("error").is_some()
        || resp_body
            .get("error_category")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("internal") || s == "error")
        || resp_body.pointer("/error/type").and_then(|v| v.as_str()) == Some("internal_error")
    {
        let raw = serde_json::to_string_pretty(&resp_body)
            .unwrap_or_else(|_| "<serialize failed>".to_string());
        let msg = format!(
            "error_envelope (status={}); do not treat agent_operation_guide as answer",
            resp_status
        );
        mark_failure(&mut outcome, msg.clone());
        eprintln!("  FAIL: {msg}");
        let preview: String = raw.chars().take(400).collect();
        eprintln!("  raw: {preview}");
        if let Some(v2) = v2 {
            v2.record_infra(idx + 1, example, subset_name, "error_envelope");
        }
        return outcome;
    }
    let chat: ChatResponse = match resp.into_business() {
        Ok(c) => c,
        Err(e) => {
            let raw = serde_json::to_string_pretty(&resp_body)
                .unwrap_or_else(|_| "<serialize failed>".to_string());
            mark_failure(&mut outcome, format!("parse: {e}"));
            eprintln!("  FAIL: parse error: {e}");
            let preview: String = raw.chars().take(500).collect();
            eprintln!("  raw response (status={}): {}", resp_status, preview);
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/e2e_output/realistic_corpus_full_eval");
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::write(dir.join(format!("q{:03}.raw.json", idx + 1)), raw);
            }
            if let Some(v2) = v2 {
                v2.record_infra(idx + 1, example, subset_name, "parse_error");
            }
            return outcome;
        }
    };
    if chat.answer.trim().is_empty() {
        let msg = "empty_answer: chat.answer is empty after successful parse".to_string();
        mark_failure(&mut outcome, msg.clone());
        eprintln!("  FAIL: {msg}");
        if let Some(v2) = v2 {
            v2.record_infra(idx + 1, example, subset_name, "empty_answer");
        }
        return outcome;
    }
    if is_code_only_answer(&chat.answer) {
        let msg = "code_block_answer: chat.answer is code-only, no prose".to_string();
        mark_failure(&mut outcome, msg.clone());
        eprintln!("  FAIL: {msg}");
        if let Some(v2) = v2 {
            v2.record_infra(idx + 1, example, subset_name, "code_block_answer");
        }
        return outcome;
    }
    let retrieved = extract_retrieved_chunks(&chat.tool_results);
    let cited = extract_cited_chunks(&chat.citations);
    if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
        "subset": subset_name,
        "query": example.query,
        "doc_scope": scope,
        "capabilities": caps,
        "answer": chat.answer.clone(),
        "citations": chat.citations.clone(),
        "sources": chat.sources.clone(),
        "tool_results_count": chat.tool_results.len(),
        "tool_trace": compact_tool_trace(&chat.tool_results),
        "mode_debug": chat.mode_debug,
    })) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/e2e_output/realistic_corpus_full_eval");
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join(format!("q{:03}.json", idx + 1)), json);
        }
    }
    let tool_trace = compact_tool_trace(&chat.tool_results);
    let chunks: Vec<String> = retrieved.contents();
    if caps.iter().any(|c| c == "rag")
        && example.expected_should_answer
        && !example.source_chunks.is_empty()
        && !chat.tool_results.iter().any(|tr| {
            tr.status == contracts::ToolStatus::Ok
                && rag_quality::harness_extract::RETRIEVAL_TOOLS.contains(&tr.tool.as_str())
        })
    {
        let msg = format!(
            "eval_bridge_miss: rag expected a retrieval-layer tool_result \
             (one of {:?}) after finalize; got tools={:?}",
            rag_quality::harness_extract::RETRIEVAL_TOOLS,
            chat.tool_results
                .iter()
                .map(|t| t.tool.as_str())
                .collect::<Vec<_>>()
        );
        eprintln!("  FAIL: {msg}");
        mark_failure(&mut outcome, msg);
    }
    let chunk_to_cite: std::collections::HashMap<String, i64> = chat
        .citations
        .iter()
        .filter_map(|c| c.chunk_id.clone().map(|id| (id, c.citation_id)))
        .collect();
    let answer = rewrite_citations(&chat.answer, &chunk_to_cite);
    let tool_outputs = builtin_tool_outputs(&chat.tool_results);
    let loop_obs = extract_loop_observability(&chat);
    if let Some(v2) = v2 {
        v2.score_question(
            idx + 1,
            example,
            subset_name,
            &retrieved,
            &cited,
            &chat.answer,
            &tool_outputs,
            &loop_obs,
            &tool_trace,
        )
        .await;
    }
    if let Some(expect) = example.expect_citations {
        let doc_n = chat
            .citations
            .iter()
            .filter(|c| c.chunk_id.is_some())
            .count() as u32;
        let web_n = chat
            .citations
            .iter()
            .filter(|c| {
                c.chunk_id.is_none()
                    && (c.chunk_type.as_deref() == Some("web") || c.source_locator.is_some())
            })
            .count() as u32;
        if doc_n < expect.min_doc || web_n < expect.min_web {
            let msg = format!(
                "expect_citations min_doc={} min_web={} got doc={} web={}",
                expect.min_doc, expect.min_web, doc_n, web_n
            );
            eprintln!("  FAIL: {msg}");
            mark_failure(&mut outcome, msg);
        }
    }
    if let Some(expected_tool) = example.expected_tool.as_deref() {
        let is_utility = matches!(
            expected_tool,
            "calculator" | "weather_query" | "user_context"
        );
        if is_utility {
            let ok_trace = extract_tool_trace(&chat.tool_results);
            let any_trace: Vec<String> =
                chat.tool_results.iter().map(|r| r.tool.clone()).collect();
            let status_dump: Vec<String> = chat
                .tool_results
                .iter()
                .map(|r| format!("{}:{:?}", r.tool, r.status))
                .collect();
            let score = ToolCoverageScore::score(example, &any_trace);
            if !score.covered {
                let msg = format!(
                    "expected_tool={expected_tool} not in tool_results \
                     any={any_trace:?} ok={ok_trace:?} status={status_dump:?} \
                     (G-17 utility gate)"
                );
                eprintln!("  FAIL: {msg}");
                mark_failure(&mut outcome, msg);
            } else {
                eprintln!("  tool_hit: {expected_tool} ok (any={any_trace:?} ok={ok_trace:?})");
            }
        }
    }
    let citation_indices = EvaluationMetrics::extract_citation_indices(&answer);
    let recall = EvaluationMetrics::recall_at_k(&example.query, &chunks, example, 15);
    let citation = EvaluationMetrics::citation_accuracy(&example.query, &citation_indices, example);
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

    outcome.recall = Some(recall);
    outcome.citation = Some(citation);
    outcome.halluc = Some(halluc);
    outcome.scorecard = Some(scorecard);
    outcome
}

#[tokio::test]
#[ignore = "requires real LLM + embedding API keys; run with --ignored --test-threads=1"]
async fn realistic_corpus_full_eval() {
    super::require_nightly_suite();
    // G-17 weather_query: real OPENWEATHER_API_KEY if present, else process mock.
    super::ensure_weather_defaults().await;

    // ADR-0012 eval v2 (judge-first generation metrics). Since P5, v2 is the
    // DEFAULT: it runs unless RAG_EVAL_V2=0|false explicitly opts out (one
    // transition cycle); RAG_EVAL_V2=1|true remains accepted with the same
    // meaning. RAG_EVAL_V2_ONLY=1 additionally suppresses the legacy
    // metrics_v2 scorecard aggregation/printing (design §4.1 transition
    // switches). Phase 0: v2 quality scores are report-only. The judge client
    // is built BEFORE corpus ingestion so missing credentials fail early
    // instead of producing 100+ JUDGE_ERRORs. run_id is a UTC timestamp
    // (`v2_YYYYMMDD-HHMMSS`) — one runner at a time, same chrono convention as
    // the TestContext artifact ids.
    let v2_active = !env_flag_disabled("RAG_EVAL_V2");
    let v2_only = v2_active && env_flag("RAG_EVAL_V2_ONLY");
    let mut v2 = if v2_active {
        super::load_env_from_repo_dotenv();
        let judge = eval_v2::JudgeClient::from_env().expect(
            "RAG_EVAL_V2=1 but no judge credentials: set JUDGE_LLM_* (or MEMORY_LLM_*) in avrag-rs/.env",
        );
        let run_id = format!("v2_{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        let run_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/e2e_output/rag_eval_v2")
            .join(&run_id);
        std::fs::create_dir_all(&run_dir).expect("create rag_eval_v2 run dir");
        // Judge response cache (design §4.4): shared across run ids so re-runs
        // of unchanged question/answer/context tuples skip the API call.
        let cache = eval_v2::JudgeCache::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/e2e_output/rag_eval_v2/cache"),
        );
        eprintln!(
            "[realistic_corpus] eval v2 ON (default since P5; RAG_EVAL_V2=0 opts out), artifacts → {}",
            run_dir.display()
        );
        // Circuit breaker (RAG_EVAL_V2=0 disables it along with v2): a trailing
        // run of this many consecutive non-PASS labels stops scheduling new
        // questions — a systemic break carries no information past that point.
        let breaker_threshold: usize = std::env::var("E2E_ABORT_AFTER_CONSECUTIVE_FAILS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8);
        eprintln!(
            "[realistic_corpus] circuit breaker: E2E_ABORT_AFTER_CONSECUTIVE_FAILS={} (0 disables)",
            breaker_threshold
        );
        Some(V2RunCtx {
            judge,
            run_id,
            run_dir,
            cache,
            scores: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            breaker: std::sync::Arc::new(std::sync::Mutex::new(
                ConsecutiveNonPassBreaker::new(breaker_threshold),
            )),
        })
    } else {
        None
    };

    // Fixed identity → stable Milvus collection prefix + PG owner across runs,
    // which is what makes corpus reuse possible:
    // - persistent PG + object store + preserved Milvus vectors (ingestion is
    //   LLM-expensive — nothing may be thrown away at teardown);
    // - first run ingests and writes the corpus cache;
    // - every later run reuses the cache by default (E2E_FORCE_INGEST=1 to
    //   force a fresh ingest);
    // - E2E_START_AT=N → resume at question N (fail-fast iteration loop).
    unsafe {
        std::env::set_var("E2E_PRESERVE_MILVUS_ON_DROP", "1");
    }
    let infra = super::super::test_context::PersistentSmokeInfra {
        postgres_url: super::super::setup::resolve_persistent_smoke_postgres_url().await,
        object_store_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/e2e_output/realistic_object_store"),
    };
    eprintln!(
        "[realistic_corpus] persistent infra: pg={} object_store={}",
        infra.postgres_url,
        infra.object_store_path.display()
    );
    let identity = Some((
        crate::product_e2e::DEFAULT_TEST_ORG_ID.to_string(),
        crate::product_e2e::DEFAULT_TEST_USER_ID.to_string(),
    ));
    // Use the PDF profile for longer ingestion timeout (large corpus).
    let mut ctx = TestContext::new_with_real_llm_pdf_persistent_corpus(identity, &infra).await;
    // Fixed realistic-corpus identity gets internal plan `e2e` (unlimited rolling
    // 5h/7d). Reuses already-ingested docs; does not change free-tier product
    // defaults for other users. See grant_e2e_unlimited_quota docs.
    ctx.grant_e2e_unlimited_quota(crate::product_e2e::DEFAULT_TEST_USER_ID)
        .await
        .expect("grant e2e unlimited quota for fixed test identity");

    // 2026-08-02 起灌原件（新解析管线路由：PDF→liteparse、Office→office-direct、
    // 文本/代码→markitdown）。docx/xlsx/pdf 原件在 fixtures 内；adr 两篇原生 md。
    // 超时按原件规模放宽：office-direct/struct_tables/LLM profile 使 docx/xlsx
    // 明显慢于派生 txt（实测 thesis.docx 单文档 struct_tables≈5min+materialize≈4min）。
    let corpus_files = [
        ("thesis_y_refrigeration.docx", 1800),        // 484KB docx, thesis（实测超 600s）
        ("adr-0004-rag-agent-loop.md", 120),         // 4.8KB MD
        ("adr-0009-codegen-sandbox-bridge.md", 300), // 13.6KB MD (materialize+summary LLM can exceed 120s)
        ("consulting_platform_network_effects.docx", 900), // 45KB docx
        ("consulting_compensation_design.docx", 600), // 91KB docx
        ("huawei_ipd_370_activities.xlsx", 1800),     // 90KB xlsx, 370 行大表 struct 重
        ("baiyao_it_planning.pdf", 900),             // 1.9MB PDF (text, liteparse)
        // v4 增量语料（智遥咨询 3 篇，OneDrive 原件已入库 fixtures）
        ("consulting_rbf_drc.docx", 600),       // 23KB docx, 滴灌通&RBF
        ("consulting_prepared_food.docx", 600), // 74KB docx, 预制菜
        ("consulting_craftsman_paradox.docx", 600), // 18KB docx, 手艺人模式
    ];
    // v3 scope keys — parallel to `corpus_files` order (doc_scope_hint → ids).
    let scope_keys = [
        "thesis",
        "adr4",
        "adr9",
        "consulting_platform",
        "consulting_compensation",
        "ipd",
        "baiyao",
        "rbf",
        "prepared_food",
        "craftsman",
    ];

    let cache_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/realistic_corpus_cache.json");
    // Progressive per-document cache: every successfully ingested doc is
    // persisted immediately, so a crash/timeout mid-ingestion never costs the
    // already-ingested (LLM-expensive) documents again.
    let mut cache: serde_json::Value = if std::env::var("E2E_FORCE_INGEST").is_err() {
        std::fs::read_to_string(&cache_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| serde_json::json!({"docs": {}}))
    } else {
        serde_json::json!({"docs": {}})
    };
    let save_cache = |cache: &serde_json::Value| {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(
            &cache_path,
            serde_json::to_string_pretty(cache).expect("serialize corpus cache"),
        )
        .expect("write corpus cache");
    };
    let workspace_id = if let Some(ws) = cache["workspace_id"].as_str() {
        eprintln!("[realistic_corpus] reusing cached workspace {ws}");
        ws.to_string()
    } else {
        let notebook = ctx
            .create_workspace("rag-quality-realistic-corpus")
            .await
            .expect("create notebook");
        cache["workspace_id"] = serde_json::json!(notebook.id);
        save_cache(&cache);
        notebook.id
    };
    let mut doc_ids: Vec<String> = Vec::new();
    for (filename, timeout_secs) in &corpus_files {
        if let Some(id) = cache["docs"][filename].as_str() {
            eprintln!("[realistic_corpus] reusing {filename} (doc_id={id})");
            doc_ids.push(id.to_string());
            continue;
        }
        eprintln!("[realistic_corpus] uploading {filename} ...");
        // 2026-08-02 起灌原件（docx/xlsx/pdf 二进制）：`upload_document_to_notebook`
        // 走 `load_fixture`(read_to_string) 仅支持文本；原件走读 bytes 的
        // `upload_file_from_path_to_notebook`。txt/md 两者皆可，统一走 bytes。
        let fixture_abs = super::super::setup::fixture_path(filename)
            .unwrap_or_else(|e| panic!("fixture_path {filename}: {e}"));
        let upload = ctx
            .upload_file_from_path_to_notebook(
                fixture_abs.to_str().expect("fixture path utf-8"),
                &workspace_id,
            )
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
        cache["docs"][filename] = serde_json::json!(upload.document_id);
        save_cache(&cache);
        doc_ids.push(upload.document_id);
    }
    assert_eq!(
        doc_ids.len(),
        corpus_files.len(),
        "should have {} documents ingested",
        corpus_files.len()
    );

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
    // v3: doc_scope is resolved per example from `doc_scope_hint` (default: full corpus).
    let mut recall_results = Vec::new();
    let mut citation_results = Vec::new();
    let mut hallucination_results = Vec::new();
    let mut scorecards = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    // E2E_FAIL_FAST=1: stop at the first failing case (per-question iteration).
    let fail_fast = std::env::var("E2E_FAIL_FAST").is_ok();
    // E2E_START_AT=N: resume at question N (1-based) — skip already-passed
    // questions when iterating on a fail-fast stop.
    let start_at: usize = std::env::var("E2E_START_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    // E2E_END_AT=N: inclusive last question (1-based). With START_AT=END_AT runs one Q.
    let end_at: Option<usize> = std::env::var("E2E_END_AT")
        .ok()
        .and_then(|v| v.parse().ok());
    if start_at > 1 {
        eprintln!(
            "[realistic_corpus] E2E_START_AT={start_at}: skipping first {} questions",
            start_at - 1
        );
    }
    if let Some(end) = end_at {
        eprintln!("[realistic_corpus] E2E_END_AT={end}: stop after question {end}");
    }
    // E2E_QUESTIONS="5,6,11,12": comma-separated 1-based question numbers —
    // only those run. When START_AT/END_AT are also set, the range applies as
    // an outer bound (intersection). Invalid tokens fail fast with the token
    // named (test harness — a typo must not silently drop coverage).
    let question_filter: Option<std::collections::HashSet<usize>> =
        std::env::var("E2E_QUESTIONS").ok().and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(
                trimmed
                    .split(',')
                    .map(|token| {
                        let token = token.trim();
                        token.parse::<usize>().unwrap_or_else(|_| {
                            panic!(
                                "E2E_QUESTIONS: invalid question number token {token:?} \
                                 (expected comma-separated 1-based integers)"
                            )
                        })
                    })
                    .collect(),
            )
        });
    if let Some(filter) = &question_filter {
        eprintln!(
            "[realistic_corpus] E2E_QUESTIONS: running {} filtered question(s)",
            filter.len()
        );
    }
    let mut per_subset_stats: std::collections::HashMap<String, (usize, usize, f64)> =
        std::collections::HashMap::new();

    // E2E_CONCURRENCY=N: max concurrent in-flight questions. DeepSeek Flash
    // allows 2500 concurrent requests; the real ceiling is the shared worker
    // process + embedding API, so default 8 and let the env tune it.
    let concurrency: usize = std::env::var("E2E_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    // Fail-fast in parallel mode: the first failure flips this flag; tasks that
    // have not started yet skip (weakened break — in-flight tasks still finish).
    let fail_fast_flag = Arc::new(AtomicBool::new(false));
    // TestContext is not Sync (owns oneshot senders / the worker child), so only
    // the plain reqwest client + base URL are shared across concurrent tasks.
    let http_client = ctx.http_client.clone();
    let base_url = ctx.base_url.clone();

    // Collect the (idx, example) pairs that pass the filters (same skip/break
    // semantics as the original serial loop).
    let mut run_items: Vec<(usize, &GoldenExample)> = Vec::new();
    for (idx, example) in examples.iter().enumerate() {
        if idx + 1 < start_at {
            continue;
        }
        if end_at.is_some_and(|end| idx + 1 > end) {
            break;
        }
        // E2E_QUESTIONS filter: skip (not stop — later filtered numbers may
        // still follow) when this question is not in the set.
        if let Some(filter) = &question_filter {
            if !filter.contains(&(idx + 1)) {
                continue;
            }
        }
        if example.requires_network && std::env::var("E2E_SKIP_NETWORK_CASES").is_ok() {
            eprintln!("  SKIP: requires_network and E2E_SKIP_NETWORK_CASES is set");
            continue;
        }
        run_items.push((idx, example));
    }
    eprintln!(
        "[realistic_corpus] running {} question(s) with E2E_CONCURRENCY={concurrency}",
        run_items.len()
    );

    // Concurrent evaluation: chat + judge for each question is one in-flight
    // future; `buffer_unordered` bounds how many run at once. Results are
    // re-ordered by index afterwards so the aggregate/report block is identical.
    // `#[tokio::test]` runs on the current-thread runtime, so the futures only
    // need to borrow (not own/Send) the shared inputs — no Arc required.
    let outcomes: Vec<QuestionOutcome> = futures::stream::iter(run_items.into_iter())
        .map(|(idx, example)| {
            // Bind references first so `async move` captures Copy references
            // instead of moving the outer variables out of the FnMut closure.
            let dataset = &dataset;
            let scope_keys = scope_keys.as_slice();
            let doc_ids = &doc_ids;
            let workspace_id = &workspace_id;
            let http_client = &http_client;
            let base_url = &base_url;
            let v2 = v2.as_ref();
            let fail_fast_flag = &fail_fast_flag;
            let examples_len = examples.len();
            async move {
                run_single_question(
                    idx,
                    example,
                    dataset,
                    scope_keys,
                    doc_ids,
                    workspace_id,
                    http_client,
                    base_url,
                    v2,
                    fail_fast,
                    fail_fast_flag,
                    examples_len,
                )
                .await
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    let mut outcomes = outcomes;
    outcomes.sort_by_key(|o| o.idx);
    for outcome in outcomes {
        failures.extend(outcome.failures);
        let (Some(recall), Some(citation), Some(halluc), Some(scorecard)) = (
            outcome.recall,
            outcome.citation,
            outcome.halluc,
            outcome.scorecard,
        ) else {
            continue;
        };
        let entry = per_subset_stats
            .entry(outcome.subset.clone())
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
    // ADR-0012: under RAG_EVAL_V2_ONLY=1 the legacy metrics_v2 scorecard
    // aggregation/printing is skipped (the per-question `score_query` calls in
    // the loop stay — they feed the per-question log line).
    let scorecard_summary = (!v2_only).then(|| ScorecardSummary::from_scorecards(&scorecards));

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
    if let Some(summary) = &scorecard_summary {
        print_scorecard_summary(
            "Decoupled RAG Scorecard (retrieval / selection / generation)",
            summary,
        );
    }
    // ADR-0012 eval v2 suite summary (Phase 0: report-only, never gates).
    if let Some(v2) = &v2 {
        v2.print_and_write_summary();
    }

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

    // Circuit-breaker trip: partial report is already printed above; fail the
    // run loudly so the caller (watchdog/observer) sees a fast non-zero exit.
    if let Some(v2) = &v2 {
        if v2.abort_requested() {
            panic!(
                "E2E_ABORT_AFTER_CONSECUTIVE_FAILS: circuit breaker tripped — \
                 aborted early, the report above is partial"
            );
        }
    }

    if fail_fast && !failures.is_empty() {
        panic!("E2E_FAIL_FAST: stopped at first failure: {:?}", failures[0]);
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
    let workspace_id = corpus.workspace_id.clone();
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
                idxs,
                idxs.len()
            );
            idxs.iter()
                .map(|i| &smoke_subset.examples[*i - 1])
                .collect()
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
        if is_subset {
            selected.len() >= 1
        } else {
            selected.len() >= 10
        },
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

        let probe = match chat_rag_observable_probe(&ctx, &example.query, &workspace_id, &doc_scope)
            .await
        {
            Ok(p) => p,
            Err(failure) => {
                let liveness = probe_api_liveness(&ctx).await;
                eprintln!(
                    "  FAIL: chat {} ({}): {}",
                    failure.error_category, failure.failing_stage, failure.error_chain
                );
                ctx.save_smoke_v5_probe_failure(
                    &artifact_key,
                    &failure,
                    &liveness,
                    Some(&serde_json::json!({ "query": example.query })),
                );
                failures.push((
                    example.query.clone(),
                    format!("chat {}: {}", failure.error_category, failure.error_chain),
                ));
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
    let workspace_id = corpus.workspace_id.clone();
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
            eprintln!(
                "  NOTE: requires_triplet_reingest=true (graph probes need triplet re-ingest)"
            );
        }

        let probe = match chat_rag_observable_probe(&ctx, &example.query, &workspace_id, &doc_scope)
            .await
        {
            Ok(p) => p,
            Err(failure) => {
                let liveness = probe_api_liveness(&ctx).await;
                eprintln!(
                    "  FAIL: chat {} ({}): {}",
                    failure.error_category, failure.failing_stage, failure.error_chain
                );
                ctx.save_smoke_v5_probe_failure(
                    &subset_name,
                    &failure,
                    &liveness,
                    Some(&serde_json::json!({ "query": example.query })),
                );
                failures.push((
                    example.query.clone(),
                    format!("chat {}: {}", failure.error_category, failure.error_chain),
                ));
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
            example.expected_tool, example.expected_tool_sequence, score.covered
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

    assert!(
        tool_summary.with_expectations > 0,
        "should have tool expectations"
    );
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
    let provider =
        std::env::var("TRIPLET_BENCHMARK_PROVIDER").unwrap_or_else(|_| "unknown".to_string());
    let single_doc = std::env::var("RAG_SMOKE_SINGLE_DOC").unwrap_or_default();
    assert_eq!(
        single_doc.trim(),
        "huawei_ipd_370_activities.txt",
        "benchmark requires RAG_SMOKE_SINGLE_DOC=huawei_ipd_370_activities.txt"
    );
    let token_budget =
        std::env::var("INGESTION_TRIPLET_TOKEN_BUDGET").unwrap_or_else(|_| "3000".to_string());

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
    let workspace_id = corpus.workspace_id.clone();
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
        match chat_rag_observable_probe(&ctx, &example.query, &workspace_id, &doc_scope).await {
            Ok(p) => p,
            Err(failure) => {
                let liveness = probe_api_liveness(&ctx).await;
                ctx.save_smoke_v5_probe_failure(
                    "pac05_benchmark",
                    &failure,
                    &liveness,
                    Some(&serde_json::json!({ "query": example.query })),
                );
                panic!(
                    "PAC-05 RAG probe failed: {} ({}): {}",
                    failure.error_category, failure.failing_stage, failure.error_chain
                );
            }
        };
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
    eprintln!(
        "[benchmark] answer: {}",
        answer.chars().take(200).collect::<String>()
    );

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

/// One-shot: reindex cached docs with triplet on. Default targets: ipd + baiyao
/// (thesis already reindexed). Override with `TRIPLET_REINDEX_DOCS=a.txt,b.txt`.
/// Requires object files under `realistic_object_store` and `INGESTION_TRIPLET_ENABLED=1`.
#[tokio::test]
#[ignore = "ops: reindex cached docs with triplet; not part of nightly suite"]
async fn reindex_three_cached_docs_with_triplet() {
    super::require_nightly_suite();
    unsafe {
        std::env::set_var("E2E_PRESERVE_MILVUS_ON_DROP", "1");
        std::env::set_var("INGESTION_TRIPLET_ENABLED", "1");
        std::env::set_var("RAG_QUALITY_REALISTIC_TRIPLET_ENABLED", "1");
    }

    let cache_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/realistic_corpus_cache.json");
    let cache: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cache_path).expect("read corpus cache"))
            .expect("parse corpus cache");
    let docs = cache["docs"].as_object().expect("cache.docs object");
    // Remaining after thesis completed; override via TRIPLET_REINDEX_DOCS.
    let need: Vec<String> = std::env::var("TRIPLET_REINDEX_DOCS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| {
            vec![
                "huawei_ipd_370_activities.txt".to_string(),
                "baiyao_it_planning.txt".to_string(),
            ]
        });
    let targets: Vec<(String, String)> = need
        .iter()
        .map(|name| {
            let id = docs
                .get(name.as_str())
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("cache missing {name}"))
                .to_string();
            (name.clone(), id)
        })
        .collect();

    let infra = super::super::test_context::PersistentSmokeInfra {
        postgres_url: super::super::setup::resolve_persistent_smoke_postgres_url().await,
        object_store_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/e2e_output/realistic_object_store"),
    };
    let identity = Some((
        crate::product_e2e::DEFAULT_TEST_ORG_ID.to_string(),
        crate::product_e2e::DEFAULT_TEST_USER_ID.to_string(),
    ));
    let mut ctx = TestContext::new_with_real_llm_pdf_persistent_corpus(identity, &infra).await;
    ctx.grant_e2e_unlimited_quota(crate::product_e2e::DEFAULT_TEST_USER_ID)
        .await
        .expect("grant e2e unlimited quota");

    for (name, doc_id) in &targets {
        eprintln!("[triplet-reindex] reindex {name} ({doc_id}) ...");
        let resp = ctx
            .reindex_document(doc_id)
            .await
            .unwrap_or_else(|e| panic!("reindex {name}: {e}"));
        assert!(
            (200..300).contains(&resp.status),
            "reindex {name} status={} body={}",
            resp.status,
            resp.body_json
        );
        // Large docs + triplet LLM: allow up to 15 min each.
        let status = ctx
            .wait_for_ingestion(doc_id, Duration::from_secs(900))
            .await
            .unwrap_or_else(|e| panic!("wait reindex {name}: {e}"));
        assert_eq!(
            status,
            DocumentStatus::Completed,
            "reindex did not complete for {name}"
        );
        // Optional: RLS may hide parse_runs from bare sqlx connect — never fail the ops job.
        match ctx.query_latest_backend_summary(doc_id).await {
            Ok(summary) => {
                let outputs = summary
                    .get("outputs")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let entities = outputs
                    .get("entity_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let relations = outputs
                    .get("relation_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let degrades = outputs
                    .get("graph_degrade_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                eprintln!(
                    "[triplet-reindex] {name} completed entities={entities} relations={relations} degrades={degrades}"
                );
            }
            Err(e) => {
                eprintln!("[triplet-reindex] {name} completed (backend_summary unavailable: {e})");
            }
        }
    }
    eprintln!("[triplet-reindex] done");
}
