//! Score-driven v2 labeling (design §5) and suite aggregation (design §7).
//!
//! Phase 0 semantics (design §7.2): labels and summaries are **report-only** —
//! nothing here gates or fails a run. Thresholds are the uncalibrated initial
//! values in `JudgeThresholds::default`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::judge_parse::{CorrectnessVerdict, JudgeOutput};
use super::{JudgeStatus, JudgeThresholds, LabelV2, ScoreV2};

/// Everything `label_for` needs to attribute one query (design §5 conditions).
///
/// Layer A numbers (infra flag, retrieval recall, cited∩gold hits) are
/// supplied by the caller; the judge half is the parsed output plus status.
#[derive(Debug, Clone)]
pub struct LabelInput<'a> {
    /// HTTP 5xx / empty parse / empty answer — computed by the caller (runner).
    pub has_infra_error: bool,
    pub judge_status: JudgeStatus,
    /// Whether the golden example declares evidence (`source_chunks` non-empty).
    pub gold_exists: bool,
    /// Retrieval recall@k from Layer A.
    pub retrieval_recall: f64,
    /// Golden chunks matched among the cited chunks (cited ∩ gold).
    pub cited_gold_hits: usize,
    /// Parsed judge output; expected `Some` iff `judge_status == Ok`. An `Ok`
    /// status without output is treated as a judge failure (never silent-pass).
    pub judge: Option<&'a JudgeOutput>,
    pub thresholds: &'a JudgeThresholds,
}

/// Assign the single root-cause label for a query, in design §5 priority
/// order: INFRA_ERROR → JUDGE_ERROR → RETRIEVAL_MISS → SELECTION_MISS →
/// REFUSAL_WRONG → UNGROUNDED → INCORRECT → PARTIAL → PASS.
pub fn label_for(input: &LabelInput) -> LabelV2 {
    if input.has_infra_error {
        return LabelV2::InfraError;
    }
    if input.judge_status == JudgeStatus::Error {
        return LabelV2::JudgeError;
    }
    if input.gold_exists && input.retrieval_recall == 0.0 {
        return LabelV2::RetrievalMiss;
    }
    let Some(judge) = input.judge else {
        // JudgeStatus::Ok without a parsed output is a pipeline bug; surface
        // it as a judge failure rather than guessing a quality label.
        return LabelV2::JudgeError;
    };
    let t = input.thresholds;
    let correctness = judge.answer_correctness.score;
    if input.retrieval_recall > 0.0 && input.cited_gold_hits == 0 && correctness < t.tau_correctness
    {
        return LabelV2::SelectionMiss;
    }
    if !judge.refusal.correct_for_expectation {
        return LabelV2::RefusalWrong;
    }
    if judge.faithfulness.score < t.tau_faithfulness
        && !judge.faithfulness.unsupported_claims.is_empty()
    {
        return LabelV2::Ungrounded;
    }
    if correctness < t.partial_min {
        return LabelV2::Incorrect;
    }
    if correctness < t.tau_correctness
        || judge.answer_correctness.verdict == CorrectnessVerdict::Partial
    {
        return LabelV2::Partial;
    }
    LabelV2::Pass
}

/// Per-subset breakdown (design §7.1 子集表).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubsetSummaryV2 {
    pub total: usize,
    pub judge_ok: usize,
    pub mean_answer_correctness: f64,
    pub mean_faithfulness: f64,
    pub mean_answer_relevancy: f64,
    pub mean_retrieval_recall_at_k: f64,
    pub label_counts: BTreeMap<LabelV2, usize>,
}

/// Suite-level aggregation (design §7.1 summary.json shape). Judge means are
/// computed over judge-ok queries only; retrieval recall and the label
/// histogram cover every query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuiteSummaryV2 {
    pub total: usize,
    pub judge_ok: usize,
    pub judge_error: usize,
    pub mean_answer_correctness: f64,
    pub mean_faithfulness: f64,
    pub mean_answer_relevancy: f64,
    pub mean_retrieval_recall_at_k: f64,
    pub label_counts: BTreeMap<LabelV2, usize>,
    pub subsets: BTreeMap<String, SubsetSummaryV2>,
}

/// Running accumulator shared by the suite and per-subset summaries.
#[derive(Default)]
struct Accum {
    total: usize,
    judge_ok: usize,
    correctness_sum: f64,
    faithfulness_sum: f64,
    relevancy_sum: f64,
    retrieval_recall_sum: f64,
    label_counts: BTreeMap<LabelV2, usize>,
}

impl Accum {
    fn push(&mut self, score: &ScoreV2) {
        self.total += 1;
        self.retrieval_recall_sum += score.retrieval.recall;
        *self.label_counts.entry(score.label).or_insert(0) += 1;
        if score.judge_status == JudgeStatus::Ok {
            if let Some(judge) = &score.judge {
                self.judge_ok += 1;
                self.correctness_sum += judge.answer_correctness.score;
                self.faithfulness_sum += judge.faithfulness.score;
                self.relevancy_sum += judge.answer_relevancy.score;
            }
        }
    }

    fn mean_over_judge_ok(sum: f64, judge_ok: usize) -> f64 {
        sum / judge_ok.max(1) as f64
    }

    fn mean_over_total(sum: f64, total: usize) -> f64 {
        sum / total.max(1) as f64
    }

    fn into_subset(self) -> SubsetSummaryV2 {
        SubsetSummaryV2 {
            total: self.total,
            judge_ok: self.judge_ok,
            mean_answer_correctness: Self::mean_over_judge_ok(self.correctness_sum, self.judge_ok),
            mean_faithfulness: Self::mean_over_judge_ok(self.faithfulness_sum, self.judge_ok),
            mean_answer_relevancy: Self::mean_over_judge_ok(self.relevancy_sum, self.judge_ok),
            mean_retrieval_recall_at_k: Self::mean_over_total(
                self.retrieval_recall_sum,
                self.total,
            ),
            label_counts: self.label_counts,
        }
    }
}

impl SuiteSummaryV2 {
    /// Aggregate per-query scores into the suite summary. Queries with
    /// `judge_status == Error` (or a missing output) are excluded from the
    /// judge means but still counted in `total`, `judge_error`, the retrieval
    /// mean, and the label histogram.
    pub fn from_scores(scores: &[ScoreV2]) -> Self {
        let mut suite = Accum::default();
        let mut subsets: BTreeMap<String, Accum> = BTreeMap::new();
        for score in scores {
            suite.push(score);
            subsets.entry(score.subset.clone()).or_default().push(score);
        }
        let judge_error = suite.total - suite.judge_ok;
        SuiteSummaryV2 {
            total: suite.total,
            judge_ok: suite.judge_ok,
            judge_error,
            mean_answer_correctness: Accum::mean_over_judge_ok(
                suite.correctness_sum,
                suite.judge_ok,
            ),
            mean_faithfulness: Accum::mean_over_judge_ok(suite.faithfulness_sum, suite.judge_ok),
            mean_answer_relevancy: Accum::mean_over_judge_ok(suite.relevancy_sum, suite.judge_ok),
            mean_retrieval_recall_at_k: Accum::mean_over_total(
                suite.retrieval_recall_sum,
                suite.total,
            ),
            label_counts: suite.label_counts,
            subsets: subsets
                .into_iter()
                .map(|(name, acc)| (name, acc.into_subset()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_v2::judge_parse::{
        CorrectnessJudgment, FaithfulnessJudgment, FaithfulnessVerdict, RelevancyJudgment,
        RefusalJudgment, SufficiencyJudgment, SufficiencyVerdict,
    };
    use crate::eval_v2::retrieval::{RetrievalScoreV2, SelectionScoreV2};
    use crate::eval_v2::{SCHEMA_VERSION, parse_judge_output};

    /// `&'static` thresholds for building `LabelInput`s (a `&Default::default()`
    /// temporary would not live long enough inside the test helpers).
    static DEFAULT_THRESHOLDS: JudgeThresholds = JudgeThresholds {
        tau_correctness: 0.7,
        tau_faithfulness: 0.7,
        partial_min: 0.4,
    };

    const JUDGE_GOOD_JSON: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/eval_v2/judge_good.json"));
    const JUDGE_UNGROUNDED_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/eval_v2/judge_ungrounded.json"
    ));
    const JUDGE_BROKEN_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/eval_v2/judge_broken.json"
    ));

    fn judge_output(
        correctness: f64,
        correctness_verdict: CorrectnessVerdict,
        faithfulness: f64,
        unsupported_claims: &[&str],
        refusal_correct: bool,
    ) -> JudgeOutput {
        JudgeOutput {
            schema_version: SCHEMA_VERSION.to_string(),
            refusal: RefusalJudgment {
                is_refusal: false,
                correct_for_expectation: refusal_correct,
                score: if refusal_correct { 1.0 } else { 0.0 },
                rationale: String::new(),
            },
            answer_correctness: CorrectnessJudgment {
                score: correctness,
                verdict: correctness_verdict,
                rationale: String::new(),
                key_points_hit: vec![],
                key_points_missed: vec![],
            },
            faithfulness: FaithfulnessJudgment {
                score: faithfulness,
                verdict: FaithfulnessVerdict::Grounded,
                unsupported_claims: unsupported_claims.iter().map(|s| s.to_string()).collect(),
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
            query: format!("{subset}-q"),
            subset: subset.to_string(),
            retrieval: RetrievalScoreV2 {
                query: format!("{subset}-q"),
                k: 15,
                recall,
                hit: recall > 0.0,
                mrr: 0.0,
                ndcg: 0.0,
                graded_recall: recall,
                graded_ndcg: 0.0,
                retrieved_count: 1,
                golden_count: 1,
                matched_golden: vec![],
                first_hit_ranks: vec![],
            },
            selection: SelectionScoreV2 {
                query: format!("{subset}-q"),
                precision: 0.0,
                recall: 0.0,
                cited_count: 0,
                golden_count: 1,
                golden_matched_in_cited: 0,
            },
            judge,
            judge_status: status,
            label,
            reference_answer: Some("ref".to_string()),
            model_answer: Some("ans".to_string()),
        }
    }

    fn base_input(judge: &JudgeOutput) -> LabelInput<'_> {
        LabelInput {
            has_infra_error: false,
            judge_status: JudgeStatus::Ok,
            gold_exists: true,
            retrieval_recall: 1.0,
            cited_gold_hits: 1,
            judge: Some(judge),
            thresholds: &DEFAULT_THRESHOLDS,
        }
    }

    // -- label priority (design §5) ----------------------------------------

    #[test]
    fn infra_error_beats_everything() {
        let judge = judge_output(0.95, CorrectnessVerdict::Correct, 1.0, &[], true);
        let mut input = base_input(&judge);
        input.has_infra_error = true;
        input.judge_status = JudgeStatus::Error; // would alone imply JUDGE_ERROR
        input.retrieval_recall = 0.0; // would alone imply RETRIEVAL_MISS
        assert_eq!(label_for(&input), LabelV2::InfraError);
    }

    #[test]
    fn judge_error_beats_retrieval_miss() {
        let input = LabelInput {
            has_infra_error: false,
            judge_status: JudgeStatus::Error,
            gold_exists: true,
            retrieval_recall: 0.0,
            cited_gold_hits: 0,
            judge: None,
            thresholds: &DEFAULT_THRESHOLDS,
        };
        assert_eq!(label_for(&input), LabelV2::JudgeError);
    }

    #[test]
    fn retrieval_miss_beats_incorrect() {
        // Correctness 0.1 would alone imply INCORRECT, but recall == 0 with
        // gold wins priority.
        let judge = judge_output(0.1, CorrectnessVerdict::Incorrect, 1.0, &[], true);
        let mut input = base_input(&judge);
        input.retrieval_recall = 0.0;
        input.cited_gold_hits = 0;
        assert_eq!(label_for(&input), LabelV2::RetrievalMiss);
    }

    #[test]
    fn selection_miss_when_retrieved_but_nothing_golden_cited_and_correctness_low() {
        let judge = judge_output(0.5, CorrectnessVerdict::Partial, 1.0, &[], true);
        let mut input = base_input(&judge);
        input.retrieval_recall = 0.5;
        input.cited_gold_hits = 0;
        assert_eq!(label_for(&input), LabelV2::SelectionMiss);
    }

    #[test]
    fn refusal_wrong_beats_ungrounded() {
        let judge = judge_output(0.9, CorrectnessVerdict::Correct, 0.2, &["员工638人"], false);
        let input = base_input(&judge);
        assert_eq!(label_for(&input), LabelV2::RefusalWrong);
    }

    #[test]
    fn ungrounded_needs_low_faithfulness_with_unsupported_claims() {
        let judge = judge_output(0.95, CorrectnessVerdict::Correct, 0.3, &["员工638人"], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Ungrounded);
        // Low faithfulness without named unsupported claims is not UNGROUNDED.
        let judge = judge_output(0.95, CorrectnessVerdict::Correct, 0.3, &[], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Pass);
    }

    #[test]
    fn incorrect_below_partial_min_partial_in_between() {
        let judge = judge_output(0.2, CorrectnessVerdict::Incorrect, 1.0, &[], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Incorrect);

        let judge = judge_output(0.5, CorrectnessVerdict::Partial, 1.0, &[], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Partial);
    }

    #[test]
    fn partial_by_verdict_even_with_high_score() {
        let judge = judge_output(0.9, CorrectnessVerdict::Partial, 1.0, &[], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Partial);
    }

    #[test]
    fn pass_when_all_dimensions_clear() {
        let judge = judge_output(0.95, CorrectnessVerdict::Correct, 1.0, &[], true);
        assert_eq!(label_for(&base_input(&judge)), LabelV2::Pass);
    }

    #[test]
    fn ok_status_without_output_is_judge_error_not_silent_pass() {
        let input = LabelInput {
            judge: None,
            ..base_input_placeholder()
        };
        assert_eq!(label_for(&input), LabelV2::JudgeError);
    }

    fn base_input_placeholder() -> LabelInput<'static> {
        LabelInput {
            has_infra_error: false,
            judge_status: JudgeStatus::Ok,
            gold_exists: true,
            retrieval_recall: 1.0,
            cited_gold_hits: 1,
            judge: None,
            thresholds: &DEFAULT_THRESHOLDS,
        }
    }

    // -- fixture-driven roundtrips ------------------------------------------

    #[test]
    fn good_fixture_parses_and_labels_pass() {
        let judge = parse_judge_output(JUDGE_GOOD_JSON).unwrap();
        assert_eq!(judge.schema_version, SCHEMA_VERSION);
        assert!((judge.answer_correctness.score - 0.95).abs() < 1e-9);
        assert!((judge.faithfulness.score - 1.0).abs() < 1e-9);
        assert!(judge.faithfulness.unsupported_claims.is_empty());
        let input = LabelInput {
            judge: Some(&judge),
            ..base_input_placeholder()
        };
        assert_eq!(label_for(&input), LabelV2::Pass);
    }

    #[test]
    fn ungrounded_fixture_parses_and_labels_ungrounded() {
        let judge = parse_judge_output(JUDGE_UNGROUNDED_JSON).unwrap();
        assert!((judge.answer_correctness.score - 0.8).abs() < 1e-9);
        assert!((judge.faithfulness.score - 0.3).abs() < 1e-9);
        assert!(!judge.faithfulness.unsupported_claims.is_empty());
        let input = LabelInput {
            judge: Some(&judge),
            ..base_input_placeholder()
        };
        assert_eq!(label_for(&input), LabelV2::Ungrounded);
    }

    #[test]
    fn broken_fixture_parse_error_maps_to_judge_error() {
        // Truncated JSON inside a markdown fence with trailing prose: the
        // extractor finds the fenced block but serde rejects it.
        let err = parse_judge_output(JUDGE_BROKEN_JSON).unwrap_err();
        let input = LabelInput {
            judge_status: JudgeStatus::Error,
            judge: None,
            ..base_input_placeholder()
        };
        assert_eq!(label_for(&input), LabelV2::JudgeError);
        // The error is structured, never a silent pass.
        assert!(matches!(
            err,
            crate::eval_v2::judge_parse::JudgeParseError::InvalidJson(_)
        ));
    }

    // -- suite aggregation ----------------------------------------------------

    #[test]
    fn suite_summary_means_histogram_and_subsets() {
        let scores = vec![
            score(
                "a",
                LabelV2::Pass,
                JudgeStatus::Ok,
                Some(judge_output(0.9, CorrectnessVerdict::Correct, 0.8, &[], true)),
                1.0,
            ),
            score(
                "a",
                LabelV2::Partial,
                JudgeStatus::Ok,
                Some(judge_output(0.5, CorrectnessVerdict::Partial, 0.6, &[], true)),
                0.5,
            ),
            score("b", LabelV2::JudgeError, JudgeStatus::Error, None, 0.0),
        ];
        let summary = SuiteSummaryV2::from_scores(&scores);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.judge_ok, 2);
        assert_eq!(summary.judge_error, 1);
        // Judge means over judge-ok only.
        assert!((summary.mean_answer_correctness - 0.7).abs() < 1e-9);
        assert!((summary.mean_faithfulness - 0.7).abs() < 1e-9);
        assert!((summary.mean_answer_relevancy - 1.0).abs() < 1e-9);
        // Retrieval mean over all queries (judge-error included).
        assert!((summary.mean_retrieval_recall_at_k - 0.5).abs() < 1e-9);
        // Histogram counts every query.
        assert_eq!(summary.label_counts.get(&LabelV2::Pass), Some(&1));
        assert_eq!(summary.label_counts.get(&LabelV2::Partial), Some(&1));
        assert_eq!(summary.label_counts.get(&LabelV2::JudgeError), Some(&1));

        let a = summary.subsets.get("a").unwrap();
        assert_eq!(a.total, 2);
        assert_eq!(a.judge_ok, 2);
        assert!((a.mean_answer_correctness - 0.7).abs() < 1e-9);
        assert!((a.mean_retrieval_recall_at_k - 0.75).abs() < 1e-9);
        let b = summary.subsets.get("b").unwrap();
        assert_eq!(b.total, 1);
        assert_eq!(b.judge_ok, 0);
        // No judge-ok entries in subset b → means are 0 by convention, not NaN.
        assert!((b.mean_answer_correctness - 0.0).abs() < 1e-9);
    }

    #[test]
    fn suite_summary_serializes_to_json() {
        let summary = SuiteSummaryV2::from_scores(&[score(
            "a",
            LabelV2::Pass,
            JudgeStatus::Ok,
            Some(judge_output(1.0, CorrectnessVerdict::Correct, 1.0, &[], true)),
            1.0,
        )]);
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["label_counts"]["PASS"], 1);
        assert_eq!(json["subsets"]["a"]["total"], 1);
    }
}
