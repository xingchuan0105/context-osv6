//! Evaluation runner.

use std::collections::BTreeMap;

use super::compare::{compare_eval_runs, EvalComparison};
use super::evaluator::Evaluator;
use super::metrics::compute_metrics;
use super::types::{
    EvalCase, EvalDatasetSpec, EvalFailure, EvalRun, EvalScore, EvalSummary, EvalTrigger,
    EvalTriggerConfig, MetricValue,
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

#[cfg(test)]
mod runner_tests;
