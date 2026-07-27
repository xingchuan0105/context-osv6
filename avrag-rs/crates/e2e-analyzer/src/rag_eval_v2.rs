//! Drift comparison for two RAG eval v2 reports (ADR-0012, design §8).
//!
//! Reads the `summary.json` files written by the v2 runner
//! (`e2e_output/rag_eval_v2/{run_id}/summary.json` — note the nested `summary`
//! key) and renders a Markdown drift report. Dependency-light on purpose:
//! serde only, no `rag_quality` LLM parts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Top-level shape of `rag_eval_v2/{run_id}/summary.json`.
#[derive(Debug, Deserialize)]
pub struct RunSummary {
    pub judge_model: String,
    pub schema_version: String,
    pub summary: SuitePart,
}

#[derive(Debug, Deserialize)]
pub struct SuitePart {
    pub total: usize,
    pub judge_ok: usize,
    pub judge_error: usize,
    pub mean_answer_correctness: f64,
    pub mean_faithfulness: f64,
    pub mean_answer_relevancy: f64,
    pub mean_retrieval_recall_at_k: f64,
    /// Full-stream retrieval recall mean (added 2026-07; absent in older
    /// summaries → defaults to 0).
    #[serde(default)]
    pub mean_retrieval_recall: f64,
    #[serde(default)]
    pub faithfulness_applicable: usize,
    #[serde(default)]
    pub retrieval_applicable: usize,
    #[serde(default)]
    pub label_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub subsets: BTreeMap<String, SubsetPart>,
}

#[derive(Debug, Deserialize)]
pub struct SubsetPart {
    pub total: usize,
    pub mean_answer_correctness: f64,
    pub mean_faithfulness: f64,
    pub mean_retrieval_recall_at_k: f64,
    #[serde(default)]
    pub mean_retrieval_recall: f64,
}

/// Accept either a `summary.json` path or a run directory (summary.json
/// appended).
fn resolve_summary_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("summary.json")
    } else {
        path.to_path_buf()
    }
}

/// Load one run's summary.json (file path or run directory).
pub fn load_summary(path: &Path) -> Result<RunSummary> {
    let resolved = resolve_summary_path(path);
    let raw = std::fs::read_to_string(&resolved)
        .with_context(|| format!("read {}", resolved.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", resolved.display()))
}

fn delta(baseline: f64, current: f64) -> String {
    format!("{:+.4}", current - baseline)
}

/// Render the Markdown drift report: per-metric deltas (4-decimal, signed),
/// judge_error counts, label histogram side-by-side, and new/missing subsets.
pub fn render_drift_markdown(baseline: &RunSummary, current: &RunSummary) -> String {
    let b = &baseline.summary;
    let c = &current.summary;
    let mut md = String::new();
    md.push_str("# RAG Eval v2 Drift\n\n");
    md.push_str(&format!("- baseline judge_model: `{}`\n", baseline.judge_model));
    md.push_str(&format!("- current  judge_model: `{}`\n", current.judge_model));
    md.push_str(&format!(
        "- schema_version: baseline `{}` / current `{}`\n",
        baseline.schema_version, current.schema_version
    ));
    if baseline.schema_version != current.schema_version {
        md.push_str("- ⚠️ schema versions differ — scores are not directly comparable\n");
    }
    if baseline.judge_model != current.judge_model {
        md.push_str("- ⚠️ judge models differ — score drift may be judge drift\n");
    }
    md.push('\n');

    md.push_str("## Suite metrics\n\n| metric | baseline | current | Δ |\n|---|---|---|---|\n");
    let rows: [(&str, f64, f64); 5] = [
        (
            "mean_answer_correctness",
            b.mean_answer_correctness,
            c.mean_answer_correctness,
        ),
        (
            "mean_faithfulness",
            b.mean_faithfulness,
            c.mean_faithfulness,
        ),
        (
            "mean_answer_relevancy",
            b.mean_answer_relevancy,
            c.mean_answer_relevancy,
        ),
        (
            "mean_retrieval_recall (full stream)",
            b.mean_retrieval_recall,
            c.mean_retrieval_recall,
        ),
        (
            "mean_retrieval_recall_at_k",
            b.mean_retrieval_recall_at_k,
            c.mean_retrieval_recall_at_k,
        ),
    ];
    for (name, bv, cv) in rows {
        md.push_str(&format!(
            "| {name} | {bv:.4} | {cv:.4} | {} |\n",
            delta(bv, cv)
        ));
    }
    md.push_str(&format!(
        "\njudge_error: baseline {} / current {} (must be 0 before any gate)\n",
        b.judge_error, c.judge_error
    ));
    md.push_str(&format!(
        "judge_ok: baseline {}/{} / current {}/{}\n\n",
        b.judge_ok, b.total, c.judge_ok, c.total
    ));

    md.push_str("## Label histogram\n\n| label | baseline | current | Δ |\n|---|---|---|---|\n");
    let mut labels: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    labels.extend(b.label_counts.keys().map(String::as_str));
    labels.extend(c.label_counts.keys().map(String::as_str));
    for label in labels {
        let bv = b.label_counts.get(label).copied().unwrap_or(0);
        let cv = c.label_counts.get(label).copied().unwrap_or(0);
        md.push_str(&format!(
            "| {label} | {bv} | {cv} | {} |\n",
            delta(bv as f64, cv as f64)
        ));
    }

    md.push_str("\n## Subsets\n\n");
    let new_subsets: Vec<&str> = c
        .subsets
        .keys()
        .filter(|k| !b.subsets.contains_key(*k))
        .map(String::as_str)
        .collect();
    let missing_subsets: Vec<&str> = b
        .subsets
        .keys()
        .filter(|k| !c.subsets.contains_key(*k))
        .map(String::as_str)
        .collect();
    if !new_subsets.is_empty() {
        md.push_str(&format!("New in current: {}\n\n", new_subsets.join(", ")));
    }
    if !missing_subsets.is_empty() {
        md.push_str(&format!(
            "Missing from current: {}\n\n",
            missing_subsets.join(", ")
        ));
    }
    md.push_str("| subset | n base | n cur | correctness Δ | faithfulness Δ | recall Δ |\n|---|---|---|---|---|---|\n");
    for (name, bs) in &b.subsets {
        if let Some(cs) = c.subsets.get(name) {
            md.push_str(&format!(
                "| {name} | {} | {} | {} | {} | {} |\n",
                bs.total,
                cs.total,
                delta(bs.mean_answer_correctness, cs.mean_answer_correctness),
                delta(bs.mean_faithfulness, cs.mean_faithfulness),
                delta(bs.mean_retrieval_recall_at_k, cs.mean_retrieval_recall_at_k),
            ));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary_json(correctness: f64, faithfulness: f64, judge_error: usize, subset: &str) -> String {
        serde_json::json!({
            "judge_model": "deepseek-v4-flash",
            "schema_version": "rag_eval_judge_v2",
            "thresholds": {"tau_correctness": 0.7, "tau_faithfulness": 0.7, "partial_min": 0.4},
            "summary": {
                "total": 2,
                "judge_ok": 2 - judge_error,
                "judge_error": judge_error,
                "mean_answer_correctness": correctness,
                "mean_faithfulness": faithfulness,
                "mean_answer_relevancy": 0.9,
                "mean_retrieval_recall_at_k": 0.8,
                "label_counts": {"PASS": 1, "PARTIAL": 1},
                "subsets": {
                    subset: {
                        "total": 2,
                        "judge_ok": 2,
                        "mean_answer_correctness": correctness,
                        "mean_faithfulness": faithfulness,
                        "mean_retrieval_recall_at_k": 0.8,
                        "label_counts": {"PASS": 1}
                    }
                }
            }
        })
        .to_string()
    }

    #[test]
    fn drift_report_computes_signed_deltas() {
        let baseline: RunSummary =
            serde_json::from_str(&summary_json(0.8, 0.7, 0, "thesis_factual")).unwrap();
        let current: RunSummary =
            serde_json::from_str(&summary_json(0.9, 0.65, 1, "thesis_factual")).unwrap();
        let md = render_drift_markdown(&baseline, &current);
        assert!(md.contains("| mean_answer_correctness | 0.8000 | 0.9000 | +0.1000 |"));
        assert!(md.contains("| mean_faithfulness | 0.7000 | 0.6500 | -0.0500 |"));
        assert!(md.contains("judge_error: baseline 0 / current 1"));
        assert!(md.contains("| PASS | 1 | 1 | +0.0000 |"));
        assert!(md.contains("| thesis_factual | 2 | 2 | +0.1000 | -0.0500 | +0.0000 |"));
    }

    #[test]
    fn drift_report_lists_new_and_missing_subsets() {
        let baseline: RunSummary =
            serde_json::from_str(&summary_json(0.8, 0.7, 0, "old_subset")).unwrap();
        let current: RunSummary =
            serde_json::from_str(&summary_json(0.8, 0.7, 0, "new_subset")).unwrap();
        let md = render_drift_markdown(&baseline, &current);
        assert!(md.contains("New in current: new_subset"));
        assert!(md.contains("Missing from current: old_subset"));
    }

    #[test]
    fn load_summary_accepts_file_or_dir() {
        let dir = std::env::temp_dir().join(format!("rag_eval_v2_drift_test_{}", std::process::id()));
        let run_dir = dir.join("v2_test");
        std::fs::create_dir_all(&run_dir).unwrap();
        let path = run_dir.join("summary.json");
        std::fs::write(&path, summary_json(0.8, 0.7, 0, "s")).unwrap();

        // Directory input resolves to <dir>/summary.json.
        let from_dir = load_summary(&run_dir).unwrap();
        assert!((from_dir.summary.mean_answer_correctness - 0.8).abs() < 1e-9);
        // Direct file input works too.
        let from_file = load_summary(&path).unwrap();
        assert_eq!(from_file.judge_model, "deepseek-v4-flash");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_summary_shape_without_new_fields_still_parses() {
        // Pre-2026-07 summaries lack mean_retrieval_recall /
        // faithfulness_applicable / retrieval_applicable — defaults fill in.
        let old = r#"{
            "judge_model": "deepseek-v4-flash",
            "schema_version": "rag_eval_judge_v2",
            "summary": {
                "total": 1,
                "judge_ok": 1,
                "judge_error": 0,
                "mean_answer_correctness": 1.0,
                "mean_faithfulness": 1.0,
                "mean_answer_relevancy": 1.0,
                "mean_retrieval_recall_at_k": 1.0,
                "label_counts": {"PASS": 1},
                "subsets": {}
            }
        }"#;
        let parsed: RunSummary = serde_json::from_str(old).unwrap();
        assert!((parsed.summary.mean_retrieval_recall_at_k - 1.0).abs() < 1e-9);
        assert!((parsed.summary.mean_retrieval_recall - 0.0).abs() < 1e-9);
        assert_eq!(parsed.summary.retrieval_applicable, 0);
    }
}
