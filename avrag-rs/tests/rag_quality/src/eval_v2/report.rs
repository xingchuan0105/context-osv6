//! Human-readable v2 reports (design §7.1 outputs, §15 per-question template).
//!
//! Pure render functions — the runner owns all I/O. Everything produced here
//! is Phase-0 report-only: no gating, no thresholds enforced.

use super::aggregate::SuiteSummaryV2;
use super::judge_parse::CorrectnessVerdict;
use super::judge_prompt::SCHEMA_VERSION;
use super::{JudgeThresholds, ScoreV2};

fn correctness_verdict_str(v: CorrectnessVerdict) -> &'static str {
    match v {
        CorrectnessVerdict::Correct => "correct",
        CorrectnessVerdict::Partial => "partial",
        CorrectnessVerdict::Incorrect => "incorrect",
        CorrectnessVerdict::NotApplicable => "not_applicable",
        CorrectnessVerdict::Unknown => "unknown",
    }
}

/// Collapse whitespace so one logical string stays on one report line.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fmt_opt(value: Option<&String>) -> String {
    value.map(|s| one_line(s)).unwrap_or_else(|| "—".to_string())
}

/// Render `summary.md`: header block, suite means, label histogram, per-subset
/// table, then per-question blocks in the design §15 template. `n` in `Q{n}` is
/// the 1-based position in `scores` (the runner pushes in question order).
pub fn render_summary_md(
    run_id: &str,
    judge_model: &str,
    thresholds: &JudgeThresholds,
    scores: &[ScoreV2],
    summary: &SuiteSummaryV2,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# RAG Eval v2 — {run_id}\n\n"));
    md.push_str(&format!("- judge_model: `{judge_model}`\n"));
    md.push_str(&format!("- schema_version: `{SCHEMA_VERSION}`\n"));
    md.push_str(&format!(
        "- thresholds (initial, uncalibrated): τ_c={} τ_f={} partial_min={}\n",
        thresholds.tau_correctness, thresholds.tau_faithfulness, thresholds.partial_min
    ));
    md.push_str("- **Phase 0: report-only — no quality gate applied (design §7.2)**\n\n");

    md.push_str("## Suite\n\n");
    md.push_str("| metric | mean |\n|---|---|\n");
    md.push_str(&format!(
        "| answer_correctness (judge-ok n={}) | {:.4} |\n",
        summary.judge_ok, summary.mean_answer_correctness
    ));
    md.push_str(&format!(
        "| faithfulness (applicable n={}) | {:.4} |\n",
        summary.faithfulness_applicable, summary.mean_faithfulness
    ));
    md.push_str(&format!(
        "| answer_relevancy (judge-ok) | {:.4} |\n",
        summary.mean_answer_relevancy
    ));
    let k = scores.first().map(|s| s.retrieval.k).unwrap_or(15);
    md.push_str(&format!(
        "| retrieval recall (full stream, n={}) | {:.4} |\n",
        summary.retrieval_applicable, summary.mean_retrieval_recall
    ));
    md.push_str(&format!(
        "| retrieval recall@{k} (top-k view, n={}) | {:.4} |\n",
        summary.retrieval_applicable, summary.mean_retrieval_recall_at_k
    ));
    md.push_str(&format!(
        "\njudge calls: ok={} error={} (JUDGE_ERROR must be 0 before any gate)\n\n",
        summary.judge_ok, summary.judge_error
    ));

    md.push_str("## Label histogram\n\n| label | count |\n|---|---|\n");
    for (label, count) in &summary.label_counts {
        md.push_str(&format!("| {} | {} |\n", label.as_str(), count));
    }

    md.push_str("\n## Per-subset\n\n");
    md.push_str("| subset | total | judge_ok | correctness | faithfulness | relevancy | recall | recall@k | labels |\n|---|---|---|---|---|---|---|---|---|\n");
    for (name, s) in &summary.subsets {
        let labels = s
            .label_counts
            .iter()
            .map(|(l, c)| format!("{}={c}", l.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!(
            "| {name} | {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {labels} |\n",
            s.total,
            s.judge_ok,
            s.mean_answer_correctness,
            s.mean_faithfulness,
            s.mean_answer_relevancy,
            s.mean_retrieval_recall,
            s.mean_retrieval_recall_at_k,
        ));
    }

    // Per-question blocks (design §15 human-readable template). Judge-error
    // questions show the error instead of judge scores.
    md.push_str("\n## Per-question\n\n");
    for (i, s) in scores.iter().enumerate() {
        md.push_str(&format!(
            "### Q{} [{}] label={}\n\n",
            i + 1,
            s.subset,
            s.label.as_str()
        ));
        md.push_str(&format!(
            "- retrieval: recall={:.2} (@{}={:.2}) hit={}\n",
            s.retrieval.recall, s.retrieval.k, s.retrieval.recall_at_k, s.retrieval.hit
        ));
        md.push_str(&format!(
            "- selection: prec={:.2} rec={:.2}\n",
            s.selection.precision, s.selection.recall
        ));
        match &s.judge {
            Some(j) => {
                md.push_str(&format!(
                    "- judge: correctness={:.2} ({}) | faithfulness={:.2} | relevancy={:.2} | refusal={}\n",
                    j.answer_correctness.score,
                    correctness_verdict_str(j.answer_correctness.verdict),
                    j.faithfulness.score,
                    j.answer_relevancy.score,
                    if j.refusal.correct_for_expectation {
                        "ok"
                    } else {
                        "WRONG"
                    },
                ));
            }
            None => {
                md.push_str("- judge: ERROR (no parsed output; judge_status=error)\n");
            }
        }
        md.push_str(&format!(
            "- reference: {}\n",
            fmt_opt(s.reference_answer.as_ref())
        ));
        md.push_str(&format!("- answer: {}\n", fmt_opt(s.model_answer.as_ref())));
        let rationale = s
            .judge
            .as_ref()
            .map(|j| one_line(&j.answer_correctness.rationale))
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| "—".to_string());
        md.push_str(&format!("- rationale: {rationale}\n\n"));
    }
    md
}

/// Render `per_query.tsv`: one row per question, empty judge columns for
/// judge-error rows (design §7.1). `recall` is the full-stream value (gold in
/// ANY round); `recall_at_k` is the top-k view.
pub fn render_per_query_tsv(scores: &[ScoreV2]) -> String {
    let mut out = String::from(
        "n\tsubset\tlabel\tcorrectness\tfaithfulness\trelevancy\trecall\trecall_at_k\tquery\n",
    );
    for (i, s) in scores.iter().enumerate() {
        let (correctness, faithfulness, relevancy) = match &s.judge {
            Some(j) => (
                format!("{:.4}", j.answer_correctness.score),
                format!("{:.4}", j.faithfulness.score),
                format!("{:.4}", j.answer_relevancy.score),
            ),
            None => (String::new(), String::new(), String::new()),
        };
        let clean = |v: &str| v.replace(['\t', '\n', '\r'], " ");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{}\n",
            i + 1,
            clean(&s.subset),
            s.label.as_str(),
            correctness,
            faithfulness,
            relevancy,
            s.retrieval.recall,
            s.retrieval.recall_at_k,
            clean(&s.query),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_v2::aggregate::SuiteSummaryV2;
    use crate::eval_v2::judge_parse::{
        CorrectnessJudgment, FaithfulnessJudgment, FaithfulnessVerdict, JudgeOutput,
        RelevancyJudgment, RefusalJudgment, SufficiencyJudgment, SufficiencyVerdict,
    };
    use crate::eval_v2::retrieval::{RetrievalScoreV2, SelectionScoreV2};
    use crate::eval_v2::{JudgeStatus, LabelV2};

    fn judge(correctness: f64, faithfulness: f64) -> JudgeOutput {
        JudgeOutput {
            schema_version: SCHEMA_VERSION.to_string(),
            refusal: RefusalJudgment {
                is_refusal: false,
                correct_for_expectation: true,
                score: 1.0,
                rationale: String::new(),
            },
            answer_correctness: CorrectnessJudgment {
                score: correctness,
                verdict: CorrectnessVerdict::Correct,
                rationale: "语义等价，仅空格差异".to_string(),
                key_points_hit: vec![],
                key_points_missed: vec![],
            },
            faithfulness: FaithfulnessJudgment {
                score: faithfulness,
                verdict: FaithfulnessVerdict::Grounded,
                unsupported_claims: vec![],
                rationale: String::new(),
            },
            answer_relevancy: RelevancyJudgment {
                score: 1.0,
                rationale: String::new(),
            },
            context_sufficiency: SufficiencyJudgment {
                score: 1.0,
                verdict: SufficiencyVerdict::Sufficient,
                rationale: String::new(),
            },
        }
    }

    fn score(
        subset: &str,
        label: LabelV2,
        status: JudgeStatus,
        judge: Option<JudgeOutput>,
        recall: f64,
    ) -> ScoreV2 {
        ScoreV2 {
            query: "Y公司哪一年在大连建厂？".to_string(),
            subset: subset.to_string(),
            retrieval: RetrievalScoreV2 {
                query: "q".to_string(),
                k: 15,
                recall,
                hit: recall > 0.0,
                recall_at_k: recall,
                hit_at_k: recall > 0.0,
                mrr: 1.0,
                ndcg: 1.0,
                graded_recall: recall,
                graded_ndcg: 1.0,
                retrieved_count: 3,
                golden_count: 1,
                matched_golden: vec![0],
                first_hit_ranks: vec![0],
            },
            selection: SelectionScoreV2 {
                query: "q".to_string(),
                precision: 0.5,
                recall,
                cited_count: 2,
                golden_count: 1,
                golden_matched_in_cited: 1,
            },
            judge,
            judge_status: status,
            label,
            reference_answer: Some("Y公司2019年在大连建厂。".to_string()),
            model_answer: Some("2019 年，Y公司在大连投资建厂。".to_string()),
            context_source: crate::eval_v2::ContextSource::Cited,
            expect_no_retrieval: false,
        }
    }

    #[test]
    fn summary_md_renders_header_means_and_question_blocks() {
        let scores = vec![
            score(
                "thesis_factual",
                LabelV2::Pass,
                JudgeStatus::Ok,
                Some(judge(0.95, 1.0)),
                1.0,
            ),
            score("thesis_factual", LabelV2::JudgeError, JudgeStatus::Error, None, 0.0),
        ];
        let summary = SuiteSummaryV2::from_scores(&scores);
        let md = render_summary_md(
            "v2_20260727-010000",
            "deepseek-v4-flash",
            &JudgeThresholds::default(),
            &scores,
            &summary,
        );
        assert!(md.contains("# RAG Eval v2 — v2_20260727-010000"));
        assert!(md.contains("judge_model: `deepseek-v4-flash`"));
        assert!(md.contains("report-only"));
        assert!(md.contains("| PASS | 1 |"));
        assert!(md.contains("| JUDGE_ERROR | 1 |"));
        assert!(md.contains("### Q1 [thesis_factual] label=PASS"));
        assert!(md.contains("correctness=0.95 (correct) | faithfulness=1.00 | relevancy=1.00 | refusal=ok"));
        assert!(md.contains("- reference: Y公司2019年在大连建厂。"));
        assert!(md.contains("- answer: 2019 年，Y公司在大连投资建厂。"));
        assert!(md.contains("- rationale: 语义等价，仅空格差异"));
        assert!(md.contains("### Q2 [thesis_factual] label=JUDGE_ERROR"));
        assert!(md.contains("- judge: ERROR"));
        // Per-subset table includes the subset row.
        assert!(md.contains("| thesis_factual | 2 | 1 |"));
    }

    #[test]
    fn per_query_tsv_has_header_and_empty_judge_fields_on_error() {
        let scores = vec![
            score(
                "a",
                LabelV2::Pass,
                JudgeStatus::Ok,
                Some(judge(0.9, 0.8)),
                1.0,
            ),
            score("b", LabelV2::JudgeError, JudgeStatus::Error, None, 0.0),
        ];
        let tsv = render_per_query_tsv(&scores);
        let mut lines = tsv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "n\tsubset\tlabel\tcorrectness\tfaithfulness\trelevancy\trecall\trecall_at_k\tquery"
        );
        let row1: Vec<&str> = lines.next().unwrap().split('\t').collect();
        assert_eq!(row1.len(), 9);
        assert_eq!(row1[0], "1");
        assert_eq!(row1[2], "PASS");
        assert_eq!(row1[3], "0.9000");
        assert_eq!(row1[6], "1.0000");
        assert_eq!(row1[7], "1.0000");
        let row2: Vec<&str> = lines.next().unwrap().split('\t').collect();
        assert_eq!(row2[2], "JUDGE_ERROR");
        assert_eq!(row2[3], "");
        assert_eq!(row2[4], "");
        assert_eq!(row2[5], "");
        assert_eq!(row2[6], "0.0000");
    }
}
