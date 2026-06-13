//! Quality and system metric computation helpers.

use super::types::{EvalRun, MetricValue, QualityMetric, SystemMetric};

/// Compute quality and system metrics from an EvalRun.
///
/// Derives what it can from the `EvalRun` / `AgentRunResult` fields;
/// metrics that require data not yet collected (e.g. cost, replay) are
/// emitted with a value of `0.0` and no target.
pub(crate) fn compute_metrics(run: &EvalRun) -> (Vec<MetricValue>, Vec<MetricValue>) {
    let mut quality = Vec::new();
    let mut system = Vec::new();

    // ---- Quality metrics ----

    // TaskCompletionRate = pass_rate (already computed).
    let total_cases = run.cases.len();
    let unique_failed: std::collections::HashSet<_> = run
        .failures
        .iter()
        .filter(|f| f.case_id != "__overall__")
        .map(|f| f.case_id.clone())
        .collect();
    let passed = total_cases.saturating_sub(unique_failed.len());
    let task_completion_rate = if total_cases == 0 {
        0.0
    } else {
        passed as f64 / total_cases as f64
    };
    quality.push(MetricValue {
        metric: QualityMetric::TaskCompletionRate.name().to_string(),
        value: task_completion_rate,
        target: Some(0.8),
    });

    // CitationAccuracy: average of citation_precision scores when available.
    let citation_scores: Vec<f64> = run
        .cases
        .iter()
        .flat_map(|c| c.scores.iter())
        .filter(|s| s.metric == "citation_precision" || s.metric == "citation_recall")
        .map(|s| s.score)
        .collect();
    if !citation_scores.is_empty() {
        let avg = citation_scores.iter().sum::<f64>() / citation_scores.len() as f64;
        quality.push(MetricValue {
            metric: QualityMetric::CitationAccuracy.name().to_string(),
            value: avg,
            target: Some(0.75),
        });
    }

    // HallucinationRate: fraction of cases with a hallucination score below threshold.
    let hallucination_scores: Vec<f64> = run
        .cases
        .iter()
        .flat_map(|c| c.scores.iter())
        .filter(|s| s.metric == "hallucination")
        .map(|s| s.score)
        .collect();
    if !hallucination_scores.is_empty() {
        // Higher hallucination score = more hallucination detected; rate = average.
        let avg = hallucination_scores.iter().sum::<f64>() / hallucination_scores.len() as f64;
        quality.push(MetricValue {
            metric: QualityMetric::HallucinationRate.name().to_string(),
            value: avg,
            target: Some(0.1),
        });
    }

    // ---- System metrics ----

    // ToolSuccessRate: from tool_calls status across all cases.
    let total_tool_calls: usize = run.cases.iter().map(|c| c.result.tool_calls.len()).sum();
    let ok_tool_calls: usize = run
        .cases
        .iter()
        .flat_map(|c| c.result.tool_calls.iter())
        .filter(|tc| tc.status == contracts::ToolStatus::Ok)
        .count();
    if total_tool_calls > 0 {
        system.push(MetricValue {
            metric: SystemMetric::ToolSuccessRate.name().to_string(),
            value: ok_tool_calls as f64 / total_tool_calls as f64,
            target: Some(0.95),
        });
    }

    // Latency percentiles from total_elapsed_ms.
    let mut latencies: Vec<u64> = run
        .cases
        .iter()
        .filter_map(|c| c.result.total_elapsed_ms)
        .collect();
    if !latencies.is_empty() {
        latencies.sort_unstable();
        let p50_idx = (latencies.len() as f64 * 0.50) as usize;
        let p95_idx = ((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1);
        let p99_idx = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
        system.push(MetricValue {
            metric: SystemMetric::LatencyP50.name().to_string(),
            value: latencies[p50_idx] as f64,
            target: Some(2000.0),
        });
        system.push(MetricValue {
            metric: SystemMetric::LatencyP95.name().to_string(),
            value: latencies[p95_idx] as f64,
            target: Some(5000.0),
        });
        system.push(MetricValue {
            metric: SystemMetric::LatencyP99.name().to_string(),
            value: latencies[p99_idx] as f64,
            target: Some(10000.0),
        });
    }

    // TokenEfficiency: total tokens per case (lower is better, but we emit raw).
    let total_tokens: u64 = run
        .cases
        .iter()
        .filter_map(|c| c.result.usage.as_ref().map(|u| u.total_tokens))
        .sum();
    if total_cases > 0 && total_tokens > 0 {
        system.push(MetricValue {
            metric: SystemMetric::TokenEfficiency.name().to_string(),
            value: total_tokens as f64 / total_cases as f64,
            target: Some(2000.0),
        });
    }

    // BudgetExhaustionRate: fraction of cases where current >= max budget.
    let budget_exhausted: usize = run
        .cases
        .iter()
        .filter(|c| {
            c.result
                .budget_used
                .as_ref()
                .map(|b| b.current >= b.max && b.max > 0)
                .unwrap_or(false)
        })
        .count();
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::BudgetExhaustionRate.name().to_string(),
            value: budget_exhausted as f64 / total_cases as f64,
            target: Some(0.05),
        });
    }

    // ReplanRate: fraction of cases with at least one replan decision.
    let replanned: usize = run
        .cases
        .iter()
        .filter(|c| c.result.iterations.iter().any(|it| it.decision == "replan"))
        .count();
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::ReplanRate.name().to_string(),
            value: replanned as f64 / total_cases as f64,
            target: Some(0.2),
        });
    }

    // AvgToolCallsPerTask.
    if total_cases > 0 {
        system.push(MetricValue {
            metric: SystemMetric::AvgToolCallsPerTask.name().to_string(),
            value: total_tool_calls as f64 / total_cases as f64,
            target: Some(5.0),
        });
    }

    (quality, system)
}
