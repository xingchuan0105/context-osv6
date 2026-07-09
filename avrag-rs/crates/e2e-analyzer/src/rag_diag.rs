//! RAG diagnostic report for llm_real observability artifacts.
//!
//! This reads per-probe `response.json` + `metadata.json` artifacts and applies
//! the decoupled RAG scorecard from `rag_quality::metrics_v2`. It is intentionally
//! read-only and works on already captured runs.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use contracts::chat::ChatResponse;
use rag_quality::{
    DiagnosticLabel, GoldenDataset, GoldenExample, PerQueryScorecard, ScorecardSummary,
    extract_cited_chunks, extract_retrieved_chunks, score_query,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDiagRow {
    pub test_name: String,
    pub subset: String,
    pub query: String,
    pub label: DiagnosticLabel,
    pub retrieval_recall_at_15: f64,
    pub retrieval_hit_at_15: bool,
    pub selection_precision: f64,
    pub selection_recall: f64,
    pub refusal_correct: bool,
    pub contract_compliant: bool,
    pub substring_faithfulness: f64,
    pub retrieved_count: usize,
    pub cited_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDiagReport {
    pub run_dir: String,
    pub golden_version: String,
    pub total: usize,
    pub skipped: usize,
    pub summary: ScorecardSummary,
    pub rows: Vec<RagDiagRow>,
}

pub fn analyze_run(run_dir: &Path, golden_path: &Path) -> Result<RagDiagReport> {
    let dataset = GoldenDataset::load(golden_path)
        .with_context(|| format!("loading golden set {}", golden_path.display()))?;
    let examples = index_examples(&dataset);

    let mut rows = Vec::new();
    let mut scorecards = Vec::new();
    let mut skipped = 0;

    for entry in fs::read_dir(run_dir).with_context(|| format!("reading {}", run_dir.display()))? {
        let test_dir = entry?.path();
        if !test_dir.is_dir() {
            continue;
        }
        let response_path = test_dir.join("response.json");
        let metadata_path = test_dir.join("metadata.json");
        if !response_path.exists() || !metadata_path.exists() {
            continue;
        }

        let metadata: serde_json::Value = read_json(&metadata_path)?;
        let Some(query) = metadata_query(&metadata) else {
            skipped += 1;
            continue;
        };
        let Some((subset, example)) = examples.get(&query) else {
            skipped += 1;
            continue;
        };
        let response: ChatResponse = read_json(&response_path)?;

        let retrieved = extract_retrieved_chunks(&response.tool_results);
        let cited = extract_cited_chunks(&response.citations);
        let scorecard = score_query(&retrieved, &cited, &response.answer, example, 15);
        let test_name = metadata
            .get("test_name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                test_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();

        rows.push(row_from_scorecard(
            test_name,
            subset.clone(),
            query,
            &scorecard,
        ));
        scorecards.push(scorecard);
    }

    rows.sort_by(|a, b| a.test_name.cmp(&b.test_name));
    let summary = ScorecardSummary::from_scorecards(&scorecards);

    Ok(RagDiagReport {
        run_dir: run_dir.display().to_string(),
        golden_version: dataset.version,
        total: rows.len(),
        skipped,
        summary,
        rows,
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn metadata_query(metadata: &serde_json::Value) -> Option<String> {
    metadata
        .get("query")
        .and_then(|v| v.as_str())
        .or_else(|| {
            metadata
                .get("extra")
                .and_then(|v| v.get("query"))
                .and_then(|v| v.as_str())
        })
        .map(str::to_string)
}

fn index_examples(dataset: &GoldenDataset) -> BTreeMap<String, (String, GoldenExample)> {
    let mut out = BTreeMap::new();
    for subset in &dataset.subsets {
        for example in &subset.examples {
            out.insert(
                example.query.clone(),
                (subset.name.clone(), example.clone()),
            );
        }
    }
    out
}

fn row_from_scorecard(
    test_name: String,
    subset: String,
    query: String,
    scorecard: &PerQueryScorecard,
) -> RagDiagRow {
    RagDiagRow {
        test_name,
        subset,
        query,
        label: scorecard.label,
        retrieval_recall_at_15: scorecard.retrieval.recall,
        retrieval_hit_at_15: scorecard.retrieval.hit,
        selection_precision: scorecard.selection.precision,
        selection_recall: scorecard.selection.recall,
        refusal_correct: scorecard.refusal.correct,
        contract_compliant: scorecard.contract.compliant,
        substring_faithfulness: scorecard.faithfulness.faithfulness,
        retrieved_count: scorecard.retrieval.retrieved_count,
        cited_count: scorecard.selection.cited_count,
    }
}

pub fn render_markdown(report: &RagDiagReport) -> String {
    let mut out = String::new();
    out.push_str("# RAG Diagnostic Report\n\n");
    out.push_str(&format!("- Run: `{}`\n", report.run_dir));
    out.push_str(&format!("- Golden version: `{}`\n", report.golden_version));
    out.push_str(&format!(
        "- Total: {} analyzed, {} skipped\n\n",
        report.total, report.skipped
    ));
    out.push_str("## Scorecard\n\n");
    out.push_str(&format!(
        "- Retrieval: Recall@15 {:.2}%, Hit@15 {:.2}%, MRR {:.3}, nDCG@15 {:.3}\n",
        report.summary.retrieval_recall_at_k * 100.0,
        report.summary.retrieval_hit_at_k * 100.0,
        report.summary.retrieval_mrr,
        report.summary.retrieval_ndcg,
    ));
    out.push_str(&format!(
        "- Selection: Precision {:.2}%, Recall {:.2}%\n",
        report.summary.selection_precision * 100.0,
        report.summary.selection_recall * 100.0,
    ));
    out.push_str(&format!(
        "- Generation: Refusal Correct {:.2}%, Contract {:.2}%, Substring Faithfulness {:.2}%\n",
        report.summary.refusal_correct_rate * 100.0,
        report.summary.contract_compliance_rate * 100.0,
        report.summary.faithfulness_mean * 100.0,
    ));
    let labels = report
        .summary
        .label_counts
        .iter()
        .map(|(label, count)| format!("{}={count}", label.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("- Labels: {labels}\n\n"));

    out.push_str("## Per Query\n\n");
    out.push_str("| test | subset | label | ret_recall | sel_precision | faithfulness | query |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---|\n");
    for row in &report.rows {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {:.0}% | {:.0}% | {:.0}% | {} |\n",
            escape_md(&row.test_name),
            escape_md(&row.subset),
            row.label.as_str(),
            row.retrieval_recall_at_15 * 100.0,
            row.selection_precision * 100.0,
            row.substring_faithfulness * 100.0,
            escape_md(&row.query),
        ));
    }
    out
}

pub fn render_drift_markdown(baseline: &RagDiagReport, current: &RagDiagReport) -> String {
    let mut out = String::new();
    out.push_str("# RAG Drift Report\n\n");
    out.push_str(&format!("- Baseline: `{}`\n", baseline.run_dir));
    out.push_str(&format!("- Current: `{}`\n\n", current.run_dir));

    out.push_str("## Summary Delta\n\n");
    out.push_str("| metric | baseline | current | delta |\n");
    out.push_str("|---|---:|---:|---:|\n");
    push_delta(
        &mut out,
        "retrieval_recall_at_15",
        baseline.summary.retrieval_recall_at_k,
        current.summary.retrieval_recall_at_k,
    );
    push_delta(
        &mut out,
        "retrieval_ndcg_at_15",
        baseline.summary.retrieval_ndcg,
        current.summary.retrieval_ndcg,
    );
    push_delta(
        &mut out,
        "selection_precision",
        baseline.summary.selection_precision,
        current.summary.selection_precision,
    );
    push_delta(
        &mut out,
        "selection_recall",
        baseline.summary.selection_recall,
        current.summary.selection_recall,
    );
    push_delta(
        &mut out,
        "substring_faithfulness",
        baseline.summary.faithfulness_mean,
        current.summary.faithfulness_mean,
    );

    let paired = paired_recall_deltas(baseline, current);
    out.push_str("\n## Paired Bootstrap\n\n");
    if paired.is_empty() {
        out.push_str("No paired queries found.\n");
    } else {
        let mean_delta = paired.iter().sum::<f64>() / paired.len() as f64;
        let (lo, hi) = bootstrap_ci(&paired, 2_000);
        out.push_str(&format!(
            "- Paired queries: {}\n- Mean Recall@15 delta: {:.2}%\n- 95% bootstrap CI: [{:.2}%, {:.2}%]\n",
            paired.len(),
            mean_delta * 100.0,
            lo * 100.0,
            hi * 100.0
        ));
        if hi < 0.0 {
            out.push_str("- Interpretation: significant regression (CI entirely below 0).\n");
        } else if lo > 0.0 {
            out.push_str("- Interpretation: significant improvement (CI entirely above 0).\n");
        } else {
            out.push_str("- Interpretation: inconclusive / likely noise (CI crosses 0).\n");
        }
    }

    out.push_str("\n## Label Drift\n\n");
    out.push_str("| label | baseline | current |\n");
    out.push_str("|---|---:|---:|\n");
    let mut labels = std::collections::BTreeSet::new();
    labels.extend(baseline.summary.label_counts.keys().copied());
    labels.extend(current.summary.label_counts.keys().copied());
    for label in labels {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            label.as_str(),
            baseline
                .summary
                .label_counts
                .get(&label)
                .copied()
                .unwrap_or(0),
            current
                .summary
                .label_counts
                .get(&label)
                .copied()
                .unwrap_or(0)
        ));
    }
    out
}

fn push_delta(out: &mut String, metric: &str, baseline: f64, current: f64) {
    out.push_str(&format!(
        "| `{metric}` | {:.2}% | {:.2}% | {:+.2}% |\n",
        baseline * 100.0,
        current * 100.0,
        (current - baseline) * 100.0
    ));
}

fn paired_recall_deltas(baseline: &RagDiagReport, current: &RagDiagReport) -> Vec<f64> {
    let base = baseline
        .rows
        .iter()
        .map(|r| (r.query.as_str(), r.retrieval_recall_at_15))
        .collect::<BTreeMap<_, _>>();
    current
        .rows
        .iter()
        .filter_map(|r| {
            base.get(r.query.as_str())
                .map(|b| r.retrieval_recall_at_15 - b)
        })
        .collect()
}

fn bootstrap_ci(values: &[f64], iterations: usize) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mut seed = 0xC0FFEE_u64;
    let mut means = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let mut sum = 0.0;
        for _ in 0..values.len() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let idx = (seed as usize) % values.len();
            sum += values[idx];
        }
        means.push(sum / values.len() as f64);
    }
    means.sort_by(|a, b| a.total_cmp(b));
    let lo = means[((iterations as f64 * 0.025).floor() as usize).min(iterations - 1)];
    let hi = means[((iterations as f64 * 0.975).floor() as usize).min(iterations - 1)];
    (lo, hi)
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
        .replace('\n', " ")
        .chars()
        .take(120)
        .collect()
}
