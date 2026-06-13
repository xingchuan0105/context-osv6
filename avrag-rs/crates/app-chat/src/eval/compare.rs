//! Baseline vs candidate evaluation comparison.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::types::EvalRun;

/// Detected regression: a case or metric that degraded between baseline and candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRegression {
    /// What kind of regression was detected.
    pub kind: RegressionKind,
    /// Case ID if this is a per-case regression; None for aggregate metric regressions.
    pub case_id: Option<String>,
    /// Metric name that regressed.
    pub metric: String,
    /// Baseline score.
    pub baseline: f64,
    /// Candidate score.
    pub candidate: f64,
    /// Negative delta (candidate - baseline).
    pub delta: f64,
}

/// Classification of regression kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionKind {
    /// A single case score dropped below the threshold.
    CaseScoreDrop,
    /// A metric average dropped below the threshold.
    MetricAverageDrop,
    /// The overall score dropped below the threshold.
    OverallScoreDrop,
}

/// Per-case score change for a specific metric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseScoreChange {
    pub case_id: String,
    pub metric: String,
    pub baseline_score: f64,
    pub candidate_score: f64,
    pub delta: f64,
}

/// Comparison result between a baseline EvalRun and a candidate EvalRun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalComparison {
    pub baseline_run_id: String,
    pub candidate_run_id: String,
    /// Per-case, per-metric changes.
    pub per_case_changes: Vec<CaseScoreChange>,
    /// Per-metric average deltas.
    pub metric_deltas: BTreeMap<String, f64>,
    /// Overall score delta (candidate - baseline).
    pub overall_delta: f64,
    /// Detected regressions (empty if candidate improved or stayed flat).
    pub regressions: Vec<EvalRegression>,
}

/// Compare a candidate EvalRun against a baseline (previous version or golden set).
///
/// `regression_threshold`: a drop larger than this (in absolute terms) is flagged
/// as a regression. Default: 0.05 (5 percentage points).
pub fn compare_eval_runs(
    baseline: &EvalRun,
    candidate: &EvalRun,
    regression_threshold: f64,
) -> EvalComparison {
    let mut per_case_changes = Vec::new();
    let mut regressions = Vec::new();

    // Build baseline case lookup by case_id.
    let baseline_cases: std::collections::HashMap<_, _> = baseline
        .cases
        .iter()
        .map(|c| (c.case_id.clone(), c))
        .collect();

    for cand_case in &candidate.cases {
        let base_case = match baseline_cases.get(&cand_case.case_id) {
            Some(b) => b,
            None => continue, // new case in candidate — skip for comparison
        };

        // Compare scores by metric name.
        let cand_scores: std::collections::HashMap<_, _> = cand_case
            .scores
            .iter()
            .map(|s| (s.metric.clone(), s.score))
            .collect();
        let base_scores: std::collections::HashMap<_, _> = base_case
            .scores
            .iter()
            .map(|s| (s.metric.clone(), s.score))
            .collect();

        for (metric, cand_score) in &cand_scores {
            let base_score = base_scores.get(metric).copied().unwrap_or(0.0);
            let delta = cand_score - base_score;
            per_case_changes.push(CaseScoreChange {
                case_id: cand_case.case_id.clone(),
                metric: metric.clone(),
                baseline_score: base_score,
                candidate_score: *cand_score,
                delta,
            });

            if delta < -regression_threshold {
                regressions.push(EvalRegression {
                    kind: RegressionKind::CaseScoreDrop,
                    case_id: Some(cand_case.case_id.clone()),
                    metric: metric.clone(),
                    baseline: base_score,
                    candidate: *cand_score,
                    delta,
                });
            }
        }
    }

    // Metric average deltas.
    let base_averages = baseline
        .summary
        .as_ref()
        .map(|s| s.metric_averages.clone())
        .unwrap_or_default();
    let cand_averages = candidate
        .summary
        .as_ref()
        .map(|s| s.metric_averages.clone())
        .unwrap_or_default();

    let mut metric_deltas: BTreeMap<String, f64> = BTreeMap::new();
    let all_metrics: std::collections::HashSet<_> = base_averages
        .keys()
        .chain(cand_averages.keys())
        .cloned()
        .collect();

    for metric in all_metrics {
        let base = base_averages.get(&metric).copied().unwrap_or(0.0);
        let cand = cand_averages.get(&metric).copied().unwrap_or(0.0);
        let delta = cand - base;
        metric_deltas.insert(metric.clone(), delta);

        if delta < -regression_threshold {
            regressions.push(EvalRegression {
                kind: RegressionKind::MetricAverageDrop,
                case_id: None,
                metric,
                baseline: base,
                candidate: cand,
                delta,
            });
        }
    }

    // Overall delta.
    let base_overall = baseline
        .summary
        .as_ref()
        .map(|s| s.overall_score)
        .unwrap_or(0.0);
    let cand_overall = candidate
        .summary
        .as_ref()
        .map(|s| s.overall_score)
        .unwrap_or(0.0);
    let overall_delta = cand_overall - base_overall;

    if overall_delta < -regression_threshold {
        regressions.push(EvalRegression {
            kind: RegressionKind::OverallScoreDrop,
            case_id: None,
            metric: "overall".to_string(),
            baseline: base_overall,
            candidate: cand_overall,
            delta: overall_delta,
        });
    }

    EvalComparison {
        baseline_run_id: baseline.run_id.clone(),
        candidate_run_id: candidate.run_id.clone(),
        per_case_changes,
        metric_deltas,
        overall_delta,
        regressions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::eval::types::{EvalCase, EvalRun, EvalScore, EvalSummary};

    fn dummy_case(answer: &str, ground: Option<&str>) -> EvalCase {
        EvalCase {
            case_id: "c1".to_string(),
            request: crate::agents::runtime::AgentRequest {
                kind: crate::agents::AgentKind::Chat,
                query: "q".to_string(),
                resolved_query: "q".to_string(),
                query_resolution: None,
                notebook_id: None,
                session_id: None,
                doc_scope: vec![],
                messages: vec![],
                user_preferences: None,
                debug: false,
                stream: false,
                language: None,
                auth_context: serde_json::json!({}),
                docscope_metadata: None,
                metadata: BTreeMap::new(),
                cancellation_token: None,
                guard_pipeline: None,
                preferred_tools: vec![],
                format_hint: None,
                max_iterations: None,
            },
            result: {
                let mut r = crate::agents::runtime::AgentRunResult::default();
                r.answer = answer.to_string();
                r
            },
            ground_truth: ground.map(|s| s.to_string()),
            scores: vec![],
        }
    }
    fn make_run(run_id: &str, cases: Vec<EvalCase>, overall: f64) -> EvalRun {
        let mut metric_sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for c in &cases {
            for s in &c.scores {
                let entry = metric_sums.entry(s.metric.clone()).or_insert((0.0, 0));
                entry.0 += s.score;
                entry.1 += 1;
            }
        }
        let metric_averages: BTreeMap<String, f64> = metric_sums
            .into_iter()
            .map(|(k, (sum, count))| (k, sum / count as f64))
            .collect();

        EvalRun {
            run_id: run_id.to_string(),
            run_name: "test".to_string(),
            strategy: "ChatStrategy".to_string(),
            strategy_version: "v1".to_string(),
            started_at_ms: 0,
            completed_at_ms: Some(0),
            cases,
            summary: Some(EvalSummary {
                total_cases: 2,
                passed_cases: 0,
                failed_cases: 0,
                metric_averages,
                overall_score: overall,
            }),
            trigger: None,
            failures: vec![],
        }
    }

    fn case_with_score(case_id: &str, metric: &str, score: f64) -> EvalCase {
        let mut case = dummy_case("answer", Some("ground"));
        case.case_id = case_id.to_string();
        case.scores = vec![EvalScore {
            metric: metric.to_string(),
            score,
            explanation: None,
        }];
        case
    }

    #[test]
    fn comparison_detects_no_regression_when_improved() {
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.5)], 0.5);
        let candidate = make_run("cand", vec![case_with_score("c1", "exact_match", 0.8)], 0.8);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert!(comp.regressions.is_empty());
        assert!((comp.overall_delta - 0.3).abs() < 1e-6);
    }

    #[test]
    fn comparison_detects_case_score_regression() {
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.8)], 0.8);
        let candidate = make_run("cand", vec![case_with_score("c1", "exact_match", 0.6)], 0.6);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert_eq!(comp.regressions.len(), 3); // case + metric average + overall
        assert!(
            comp.regressions
                .iter()
                .any(|r| r.kind == RegressionKind::CaseScoreDrop)
        );
    }

    #[test]
    fn comparison_detects_overall_regression() {
        let baseline = make_run("base", vec![case_with_score("c1", "f1", 0.9)], 0.9);
        let candidate = make_run("cand", vec![case_with_score("c1", "f1", 0.7)], 0.7);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        assert!(
            comp.regressions
                .iter()
                .any(|r| r.kind == RegressionKind::OverallScoreDrop)
        );
    }

    #[test]
    fn comparison_ignores_cases_missing_in_baseline() {
        let baseline = make_run("base", vec![case_with_score("c1", "m1", 0.5)], 0.5);
        let mut candidate_cases = vec![case_with_score("c1", "m1", 0.6)];
        candidate_cases.push(case_with_score("c2", "m1", 0.4)); // new case
        let candidate = make_run("cand", candidate_cases, 0.6);
        let comp = compare_eval_runs(&baseline, &candidate, 0.05);
        // c2 is skipped because it is not in baseline.
        assert_eq!(comp.per_case_changes.len(), 1);
    }
}
