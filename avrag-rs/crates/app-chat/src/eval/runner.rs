//! Evaluation runner and built-in evaluators.

use std::collections::BTreeMap;

use super::compare::{compare_eval_runs, EvalComparison};
use super::metrics::compute_metrics;
use super::types::{
    EvalCase, EvalDatasetSpec, EvalFailure, EvalMetric, EvalRun, EvalScore, EvalSummary,
    EvalTrigger, EvalTriggerConfig, MetricValue,
};

/// Result of a trigger-based evaluation, including metrics and optional baseline comparison.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvalResult {
    /// What triggered this evaluation run.
    pub trigger: EvalTrigger,
    /// Fraction of cases that passed all thresholds.
    pub pass_rate: f64,
    /// Average latency per case in milliseconds.
    pub avg_latency_ms: u64,
    /// Average token consumption per case.
    pub avg_tokens: u64,
    /// Cases that failed (score below threshold or evaluator error).
    pub failures: Vec<EvalFailure>,
    /// Comparison against a baseline run, if one was provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<EvalComparison>,
    /// Quality metrics computed from the evaluation run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_metrics: Vec<MetricValue>,
    /// System metrics computed from the evaluation run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_metrics: Vec<MetricValue>,
}

/// Pluggable evaluator interface.
#[async_trait::async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate a single case and return a score.
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError>;
}

// ---------------------------------------------------------------------------
// Built-in evaluators
// ---------------------------------------------------------------------------

/// Exact-match evaluator (case-insensitive, after normalisation).
pub struct ExactMatchEvaluator;

#[async_trait::async_trait]
impl Evaluator for ExactMatchEvaluator {
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError> {
        let ground = case.ground_truth.as_ref().ok_or_else(|| {
            common::AppError::validation("missing_ground_truth", "ExactMatch requires ground_truth")
        })?;
        let prediction = normalize(&case.result.answer);
        let reference = normalize(ground);
        let score = if prediction == reference { 1.0 } else { 0.0 };
        Ok(EvalScore {
            metric: EvalMetric::ExactMatch.name().to_string(),
            score,
            explanation: Some(format!(
                "prediction_len={}, reference_len={}, match={}",
                prediction.len(),
                reference.len(),
                score == 1.0
            )),
        })
    }
}

/// F1 score based on whitespace token overlap.
pub struct F1Evaluator;

#[async_trait::async_trait]
impl Evaluator for F1Evaluator {
    async fn evaluate(&self, case: &EvalCase) -> Result<EvalScore, common::AppError> {
        let ground = case.ground_truth.as_ref().ok_or_else(|| {
            common::AppError::validation("missing_ground_truth", "F1 requires ground_truth")
        })?;
        let pred_tokens = tokenize(&case.result.answer);
        let ref_tokens = tokenize(ground);
        let score = compute_f1(&pred_tokens, &ref_tokens);
        Ok(EvalScore {
            metric: EvalMetric::F1.name().to_string(),
            score,
            explanation: Some(format!(
                "pred_tokens={}, ref_tokens={}, f1={:.2}",
                pred_tokens.len(),
                ref_tokens.len(),
                score
            )),
        })
    }
}

/// Run a suite of evaluators over a set of cases and produce an EvalRun.
pub async fn run_evaluation(
    run_name: impl Into<String>,
    strategy: impl Into<String>,
    strategy_version: impl Into<String>,
    cases: Vec<EvalCase>,
    evaluators: Vec<Box<dyn Evaluator>>,
) -> Result<EvalRun, common::AppError> {
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut evaluated_cases = Vec::with_capacity(cases.len());
    let mut metric_sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for mut case in cases {
        for evaluator in &evaluators {
            match evaluator.evaluate(&case).await {
                Ok(score) => {
                    if score.score >= 0.7 {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                    let entry = metric_sums.entry(score.metric.clone()).or_insert((0.0, 0));
                    entry.0 += score.score;
                    entry.1 += 1;
                    case.scores.push(score);
                }
                Err(e) => {
                    tracing::warn!(case_id = %case.case_id, error = %e, "evaluator failed");
                    failed += 1;
                }
            }
        }
        evaluated_cases.push(case);
    }

    let metric_averages: BTreeMap<String, f64> = metric_sums
        .iter()
        .map(|(k, (sum, count))| {
            (
                k.clone(),
                if *count > 0 { sum / *count as f64 } else { 0.0 },
            )
        })
        .collect();

    let overall_score = if metric_averages.is_empty() {
        0.0
    } else {
        metric_averages.values().sum::<f64>() / metric_averages.len() as f64
    };

    let completed_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let total_cases = evaluated_cases.len();

    Ok(EvalRun {
        run_id: format!("eval-{}", uuid::Uuid::new_v4()),
        run_name: run_name.into(),
        strategy: strategy.into(),
        strategy_version: strategy_version.into(),
        started_at_ms,
        completed_at_ms: Some(completed_at_ms),
        cases: evaluated_cases,
        summary: Some(EvalSummary {
            total_cases,
            passed_cases: passed,
            failed_cases: failed,
            metric_averages,
            overall_score,
        }),
        trigger: None,
        failures: vec![],
    })
}

/// Run an evaluation under a specific trigger, collecting failures and producing an EvalResult.
///
/// 1. Runs all evaluators over the cases.
/// 2. Compares scores against trigger-specific thresholds.
/// 3. Populates `EvalRun.trigger` and `EvalRun.failures`.
/// 4. If `baseline` is provided, computes an `EvalComparison` and attaches it to the result.
pub async fn run_eval_with_trigger(
    run_name: impl Into<String>,
    strategy: impl Into<String>,
    strategy_version: impl Into<String>,
    cases: Vec<EvalCase>,
    evaluators: Vec<Box<dyn Evaluator>>,
    config: EvalTriggerConfig,
    baseline: Option<&EvalRun>,
) -> Result<(EvalRun, EvalResult), common::AppError> {
    let mut run = run_evaluation(run_name, strategy, strategy_version, cases, evaluators).await?;

    run.trigger = Some(config.trigger.clone());
    run.failures.clear();

    // Collect failures: per-case per-metric under threshold.
    for case in &run.cases {
        for score in &case.scores {
            let threshold = config
                .metric_thresholds
                .get(&score.metric)
                .copied()
                .unwrap_or(config.pass_threshold);
            if score.score < threshold {
                run.failures.push(EvalFailure {
                    case_id: case.case_id.clone(),
                    metric: score.metric.clone(),
                    score: score.score,
                    threshold,
                    reason: format!(
                        "{} score {:.2} below threshold {:.2}",
                        score.metric, score.score, threshold
                    ),
                });
            }
        }
    }

    // Also flag overall score.
    if let Some(ref summary) = run.summary
        && summary.overall_score < config.pass_threshold
    {
        run.failures.push(EvalFailure {
            case_id: "__overall__".to_string(),
            metric: "overall".to_string(),
            score: summary.overall_score,
            threshold: config.pass_threshold,
            reason: format!(
                "overall score {:.2} below pass threshold {:.2}",
                summary.overall_score, config.pass_threshold
            ),
        });
    }

    // Compute pass rate.
    let total_cases = run.cases.len();
    let pass_rate = if total_cases == 0 {
        0.0
    } else {
        let _failed_count = run
            .failures
            .iter()
            .filter(|f| f.case_id != "__overall__")
            .count();
        let unique_failed_cases: std::collections::HashSet<_> = run
            .failures
            .iter()
            .filter(|f| f.case_id != "__overall__")
            .map(|f| f.case_id.clone())
            .collect();
        let passed = total_cases.saturating_sub(unique_failed_cases.len());
        passed as f64 / total_cases as f64
    };

    // Compute system metrics from case results.
    let (quality_metrics, system_metrics) = compute_metrics(&run);

    // Build EvalResult.
    let mut result = EvalResult {
        trigger: config.trigger,
        pass_rate,
        avg_latency_ms: 0, // TODO: collect per-case latency when available
        avg_tokens: 0,     // TODO: collect per-case tokens when available
        failures: run.failures.clone(),
        comparison: None,
        quality_metrics,
        system_metrics,
    };

    // If baseline provided, compute comparison.
    if let Some(base) = baseline {
        let comparison = compare_eval_runs(base, &run, 0.05);
        result.comparison = Some(comparison);
    }

    Ok((run, result))
}

pub(crate) fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(|c: char| c.is_ascii_punctuation(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn tokenize(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub(crate) fn compute_f1(pred: &[String], reference: &[String]) -> f64 {
    if pred.is_empty() && reference.is_empty() {
        return 1.0;
    }
    if pred.is_empty() || reference.is_empty() {
        return 0.0;
    }

    let pred_set: std::collections::HashSet<_> = pred.iter().collect();
    let ref_set: std::collections::HashSet<_> = reference.iter().collect();

    let overlap: std::collections::HashSet<_> = pred_set.intersection(&ref_set).collect();
    let overlap_count = overlap.len() as f64;

    let precision = overlap_count / pred_set.len() as f64;
    let recall = overlap_count / ref_set.len() as f64;

    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * (precision * recall) / (precision + recall)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

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

    #[tokio::test]
    async fn exact_match_perfect() {
        let case = dummy_case("hello world", Some("hello world"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 1.0);
    }

    #[tokio::test]
    async fn exact_match_fails_on_difference() {
        let case = dummy_case("hello world", Some("hello"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 0.0);
    }

    #[tokio::test]
    async fn exact_match_normalizes_case_and_punctuation() {
        let case = dummy_case("Hello, World!", Some("hello world"));
        let eval = ExactMatchEvaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 1.0);
    }

    #[tokio::test]
    async fn exact_match_requires_ground_truth() {
        let case = dummy_case("hello", None);
        let eval = ExactMatchEvaluator;
        let result = eval.evaluate(&case).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn f1_perfect_match() {
        let case = dummy_case("a b c", Some("a b c"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert!(score.score > 0.99);
    }

    #[tokio::test]
    async fn f1_partial_match() {
        let case = dummy_case("a b c", Some("a b d"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert!(score.score > 0.5 && score.score < 1.0);
    }

    #[tokio::test]
    async fn f1_no_overlap() {
        let case = dummy_case("x y z", Some("a b c"));
        let eval = F1Evaluator;
        let score = eval.evaluate(&case).await.unwrap();
        assert_eq!(score.score, 0.0);
    }

    #[tokio::test]
    async fn run_evaluation_computes_summary() {
        let cases = vec![
            dummy_case("hello world", Some("hello world")),
            dummy_case("foo bar", Some("foo baz")),
        ];
        let evaluators: Vec<Box<dyn Evaluator>> =
            vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
        let run = run_evaluation("test", "ChatStrategy", "v1", cases, evaluators)
            .await
            .unwrap();
        assert!(run.summary.is_some());
        let summary = run.summary.unwrap();
        assert_eq!(summary.total_cases, 2);
        assert!(summary.overall_score > 0.0);
        assert!(summary.metric_averages.contains_key("exact_match"));
        assert!(summary.metric_averages.contains_key("f1"));
    }

    #[test]
    fn eval_metric_names() {
        assert_eq!(EvalMetric::ExactMatch.name(), "exact_match");
        assert_eq!(EvalMetric::F1.name(), "f1");
        assert_eq!(EvalMetric::LlmAsJudge.name(), "llm_as_judge");
    }
    #[tokio::test]
    async fn run_eval_with_trigger_collects_failures() {
        let cases = vec![
            dummy_case("hello world", Some("hello world")), // exact_match=1.0, f1=1.0
            dummy_case("foo bar", Some("foo baz")),         // exact_match=0.0, f1<1.0
        ];
        let evaluators: Vec<Box<dyn Evaluator>> =
            vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2,
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 2,
                filter: None,
            },
            pass_threshold: 0.75,
            metric_thresholds: BTreeMap::new(),
        };
        let (run, result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert_eq!(
            run.trigger,
            Some(EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2
            })
        );
        assert!(
            !run.failures.is_empty(),
            "expected some failures below 0.75 threshold"
        );
        assert!(run.failures.iter().any(|f| f.metric == "exact_match"));
        assert!(run.failures.iter().any(|f| f.metric == "overall"));
        assert_eq!(
            result.trigger,
            EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 2
            }
        );
        assert!(result.pass_rate >= 0.0 && result.pass_rate <= 1.0);
    }

    #[tokio::test]
    async fn run_eval_with_trigger_uses_metric_thresholds() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let mut metric_thresholds = BTreeMap::new();
        metric_thresholds.insert("exact_match".to_string(), 1.01); // impossible
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::NightlyRegression {
                dataset: "test".to_string(),
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.5,
            metric_thresholds,
        };
        let (run, _result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert_eq!(
            run.trigger,
            Some(EvalTrigger::NightlyRegression {
                dataset: "test".to_string()
            })
        );
        assert!(
            run.failures
                .iter()
                .any(|f| f.metric == "exact_match" && f.threshold == 1.01)
        );
    }

    #[tokio::test]
    async fn run_eval_with_trigger_no_failures_when_all_pass() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::RedTeam {
                attack_vectors: vec![],
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.0, // everything passes
            metric_thresholds: BTreeMap::new(),
        };
        let (run, result) =
            run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
                .await
                .unwrap();
        assert!(
            run.failures.is_empty(),
            "expected no failures with threshold 0.0"
        );
        assert_eq!(result.pass_rate, 1.0);
    }

    #[tokio::test]
    async fn run_eval_with_trigger_comparison_with_baseline() {
        let cases = vec![dummy_case("hello world", Some("hello world"))];
        let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
        let config = EvalTriggerConfig {
            trigger: EvalTrigger::PreMerge {
                dataset: "test".to_string(),
                sample_size: 1,
            },
            dataset: EvalDatasetSpec {
                dataset_id: "test".to_string(),
                sample_size: 1,
                filter: None,
            },
            pass_threshold: 0.0,
            metric_thresholds: BTreeMap::new(),
        };

        // Build a baseline run with lower score.
        let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.5)], 0.5);

        let (_run, result) = run_eval_with_trigger(
            "t",
            "ChatStrategy",
            "v1",
            cases,
            evaluators,
            config,
            Some(&baseline),
        )
        .await
        .unwrap();

        assert!(
            result.comparison.is_some(),
            "expected comparison when baseline is provided"
        );
        let comp = result.comparison.unwrap();
        assert_eq!(comp.baseline_run_id, "base");
        assert!(
            comp.overall_delta > 0.0,
            "expected positive delta since candidate improved"
        );
        assert!(
            comp.regressions.is_empty(),
            "expected no regressions since candidate improved"
        );
    }
}
