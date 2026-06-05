//! Stability and trending analysis — analyze test stability across multiple runs.

use crate::models::{
    CategorySnapshot, PerfRegression, PerfTrend, StabilityRecord, TestResult, TestStatus,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze test stability across multiple runs.
///
/// `runs` is a slice of `(run_id, results)` pairs.
/// Returns `None` if fewer than 2 data points are found for the test.
pub fn analyze_stability(
    test_name: &str,
    runs: &[(String, Vec<TestResult>)],
) -> Option<StabilityRecord> {
    // Collect data points for this test across runs
    let mut data_points = Vec::new();

    for (run_id, results) in runs {
        if let Some(result) = results.iter().find(|r| r.test_name == test_name) {
            data_points.push((run_id.clone(), result));
        }
    }

    if data_points.len() < 2 {
        return None;
    }

    let total = data_points.len();
    let passed = data_points
        .iter()
        .filter(|(_, r)| r.status == TestStatus::Passed)
        .count();
    let pass_rate = passed as f64 / total as f64;
    let _flaky_rate = 1.0 - pass_rate;

    // Consecutive failures from the end (newest runs)
    let mut _consecutive_failures = 0usize;
    for (_, result) in data_points.iter().rev() {
        if result.status == TestStatus::Failed {
            _consecutive_failures += 1;
        } else {
            break;
        }
    }

    // Duration stats
    let durations: Vec<f64> = data_points.iter().map(|(_, r)| r.duration_ms as f64).collect();
    let avg_duration_ms = durations.iter().sum::<f64>() / durations.len() as f64;
    let variance = durations
        .iter()
        .map(|d| (d - avg_duration_ms).powi(2))
        .sum::<f64>()
        / durations.len() as f64;
    let stddev_duration_ms = variance.sqrt();

    // Category history (CategorySnapshot)
    let category_snapshots: Vec<CategorySnapshot> = data_points
        .iter()
        .map(|(run_id, result)| CategorySnapshot {
            run_id: run_id.clone(),
            status: result.status,
            duration_ms: result.duration_ms,
            token_usage: result.token_usage.clone(),
        })
        .collect();

    // Hard regressions: any run with duration > 20_000 ms
    let hard_regressions: Vec<PerfRegression> = data_points
        .iter()
        .filter(|(_, r)| r.duration_ms > 20_000)
        .map(|(_run_id, result)| PerfRegression {
            threshold_pct: 0.0,
            actual_pct: 0.0,
            baseline_avg: 0.0,
            current_avg: result.duration_ms as f64,
        })
        .collect();

    // Token drift: if ≥5 runs have token_usage, compute linear slope
    let token_points: Vec<(String, f64)> = data_points
        .iter()
        .filter_map(|(run_id, result)| {
            result.token_usage.as_ref().map(|tu| {
                let total = tu.prompt_tokens + tu.completion_tokens;
                (run_id.clone(), total as f64)
            })
        })
        .collect();

    let _perf_trend = if token_points.len() >= 5 {
        let slope = compute_linear_slope(&token_points);
        Some(PerfTrend {
            test_name: test_name.to_string(),
            metric: "total_tokens".to_string(),
            values: token_points.iter().map(|(_, v)| *v).collect(),
            run_ids: token_points.iter().map(|(r, _)| r.clone()).collect(),
            regression: if !hard_regressions.is_empty() {
                hard_regressions.first().cloned()
            } else {
                None
            },
            drift: if slope > 0.0 {
                Some(crate::models::DriftWarning {
                    window_size: token_points.len(),
                    stddev_multiplier: 1.0,
                    detected_at_run_id: token_points.last().map(|(r, _)| r.clone()).unwrap_or_default(),
                    description: format!("Token usage trending upward (slope ≈ {:.2})", slope),
                })
            } else {
                None
            },
        })
    } else {
        None
    };

    let last_status = data_points.last().map(|(_, r)| r.status).unwrap_or(TestStatus::Skipped);

    Some(StabilityRecord {
        test_name: test_name.to_string(),
        runs: total,
        pass_rate,
        avg_duration_ms,
        stddev_duration_ms,
        last_status,
        category_snapshots,
    })
}

/// Generate a Markdown stability report.
pub fn generate_stability_report(record: &StabilityRecord) -> String {
    let mut report = String::new();
    report.push_str(&format!("# Stability Report: {}\n\n", record.test_name));
    report.push_str(&format!("- **Runs analyzed:** {}\n", record.runs));
    report.push_str(&format!("- **Pass rate:** {:.1}%\n", record.pass_rate * 100.0));
    report.push_str(&format!(
        "- **Avg duration:** {:.0} ms (stddev: {:.0} ms)\n",
        record.avg_duration_ms, record.stddev_duration_ms
    ));
    report.push_str(&format!("- **Last status:** {:?}\n", record.last_status));

    // Compute flaky rate and consecutive failures from category_snapshots
    let total = record.category_snapshots.len();
    let passed = record
        .category_snapshots
        .iter()
        .filter(|s| s.status == TestStatus::Passed)
        .count();
    let flaky_rate = if total > 0 {
        (total - passed) as f64 / total as f64
    } else {
        0.0
    };

    let mut consecutive_failures = 0usize;
    for snap in record.category_snapshots.iter().rev() {
        if snap.status == TestStatus::Failed {
            consecutive_failures += 1;
        } else {
            break;
        }
    }

    report.push_str(&format!("- **Flaky rate:** {:.1}%\n", flaky_rate * 100.0));
    report.push_str(&format!(
        "- **Consecutive failures:** {}\n",
        consecutive_failures
    ));

    // Duration hard regressions
    let hard_regressions: Vec<_> = record
        .category_snapshots
        .iter()
        .filter(|s| s.duration_ms > 20_000)
        .collect();
    if !hard_regressions.is_empty() {
        report.push_str("\n## Duration Hard Regressions\n");
        for snap in &hard_regressions {
            report.push_str(&format!(
                "- Run `{}`: {} ms > 20_000 ms threshold\n",
                snap.run_id, snap.duration_ms
            ));
        }
    }

    // Token drift
    let token_points: Vec<(String, f64)> = record
        .category_snapshots
        .iter()
        .filter_map(|s| {
            s.token_usage.as_ref().map(|tu| {
                let total = tu.prompt_tokens + tu.completion_tokens;
                (s.run_id.clone(), total as f64)
            })
        })
        .collect();

    if token_points.len() >= 5 {
        let slope = compute_linear_slope(&token_points);
        report.push_str("\n## Token Usage Trend\n");
        report.push_str(&format!("- **Data points:** {}\n", token_points.len()));
        report.push_str(&format!("- **Linear slope:** {:.2} tokens/run\n", slope));
        if slope > 0.0 {
            report.push_str("- **Warning:** Token usage is trending upward\n");
        } else {
            report.push_str("- **Status:** Stable or improving\n");
        }
    }

    // Run history table
    report.push_str("\n## Run History\n\n");
    report.push_str("| Run ID | Status | Duration (ms) | Tokens |\n");
    report.push_str("|--------|--------|---------------|--------|\n");
    for snap in &record.category_snapshots {
        let tokens = snap
            .token_usage
            .as_ref()
            .map(|tu| format!("{}", tu.prompt_tokens + tu.completion_tokens))
            .unwrap_or_else(|| "-".to_string());
        report.push_str(&format!(
            "| {} | {:?} | {} | {} |\n",
            snap.run_id, snap.status, snap.duration_ms, tokens
        ));
    }

    report
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute linear slope using simple least squares.
/// `values` is `[(run_id, y)]` where x = 0, 1, 2, ... (run index).
fn compute_linear_slope(values: &[(String, f64)]) -> f64 {
    let n = values.len() as f64;
    if n <= 1.0 {
        return 0.0;
    }

    let sum_x: f64 = (0..values.len()).map(|i| i as f64).sum();
    let sum_y: f64 = values.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = values
        .iter()
        .enumerate()
        .map(|(i, (_, y))| i as f64 * y)
        .sum();
    let sum_x2: f64 = (0..values.len()).map(|i| (i as f64).powi(2)).sum();

    let denominator = n * sum_x2 - sum_x * sum_x;
    if denominator.abs() < f64::EPSILON {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denominator
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TokenUsage;

    fn make_result(test_name: &str, status: TestStatus, duration_ms: u64, tokens: Option<TokenUsage>) -> TestResult {
        TestResult {
            run_id: "r1".to_string(),
            test_name: test_name.to_string(),
            query: "q".to_string(),
            strategy: "s".to_string(),
            format_skill: None,
            status,
            answer_text: String::new(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: Vec::new(),
            tool_calls: Vec::new(),
            retrieval_hits: None,
            token_usage: tokens,
            duration_ms,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_flaky_detection() {
        // 5 runs: P, F, P, F, F → flaky_rate = 0.6, consecutive_failures = 2
        let runs = vec![
            (
                "run_1".to_string(),
                vec![make_result("my_test", TestStatus::Passed, 1000, None)],
            ),
            (
                "run_2".to_string(),
                vec![make_result("my_test", TestStatus::Failed, 1000, None)],
            ),
            (
                "run_3".to_string(),
                vec![make_result("my_test", TestStatus::Passed, 1000, None)],
            ),
            (
                "run_4".to_string(),
                vec![make_result("my_test", TestStatus::Failed, 1000, None)],
            ),
            (
                "run_5".to_string(),
                vec![make_result("my_test", TestStatus::Failed, 1000, None)],
            ),
        ];

        let record = analyze_stability("my_test", &runs).unwrap();
        assert_eq!(record.runs, 5);
        assert_eq!(record.pass_rate, 0.4); // 2 passed out of 5

        let report = generate_stability_report(&record);
        assert!(report.contains("Flaky rate:** 60.0%"), "Report:\n{}", report);
        assert!(report.contains("Consecutive failures:** 2"), "Report:\n{}", report);
    }

    #[test]
    fn test_duration_hard_regression() {
        let runs = vec![
            (
                "run_1".to_string(),
                vec![make_result("my_test", TestStatus::Passed, 25_000, None)],
            ),
            (
                "run_2".to_string(),
                vec![make_result("my_test", TestStatus::Passed, 10_000, None)],
            ),
        ];

        let record = analyze_stability("my_test", &runs).unwrap();
        let report = generate_stability_report(&record);
        assert!(report.contains("Duration Hard Regressions"));
        assert!(report.contains("25000"));
    }

    #[test]
    fn test_linear_slope_computation() {
        // values [10, 20, 30, 40, 50] → slope ≈ 10
        let values: Vec<(String, f64)> = vec![
            ("r1".to_string(), 10.0),
            ("r2".to_string(), 20.0),
            ("r3".to_string(), 30.0),
            ("r4".to_string(), 40.0),
            ("r5".to_string(), 50.0),
        ];
        let slope = compute_linear_slope(&values);
        assert!((slope - 10.0).abs() < 0.001, "Expected slope ≈ 10, got {}", slope);
    }

    #[test]
    fn test_linear_slope_flat() {
        let values: Vec<(String, f64)> = vec![
            ("r1".to_string(), 100.0),
            ("r2".to_string(), 100.0),
            ("r3".to_string(), 100.0),
        ];
        let slope = compute_linear_slope(&values);
        assert!(slope.abs() < 0.001, "Expected slope ≈ 0, got {}", slope);
    }

    #[test]
    fn test_insufficient_data_returns_none() {
        let runs = vec![(
            "run_1".to_string(),
            vec![make_result("my_test", TestStatus::Passed, 1000, None)],
        )];
        let result = analyze_stability("my_test", &runs);
        assert!(result.is_none(), "Expected None for single data point");
    }

    #[test]
    fn test_token_drift_warning() {
        let runs = vec![
            (
                "run_1".to_string(),
                vec![make_result(
                    "my_test",
                    TestStatus::Passed,
                    1000,
                    Some(TokenUsage {
                        prompt_tokens: 100,
                        completion_tokens: 50,
                    }),
                )],
            ),
            (
                "run_2".to_string(),
                vec![make_result(
                    "my_test",
                    TestStatus::Passed,
                    1000,
                    Some(TokenUsage {
                        prompt_tokens: 200,
                        completion_tokens: 100,
                    }),
                )],
            ),
            (
                "run_3".to_string(),
                vec![make_result(
                    "my_test",
                    TestStatus::Passed,
                    1000,
                    Some(TokenUsage {
                        prompt_tokens: 300,
                        completion_tokens: 150,
                    }),
                )],
            ),
            (
                "run_4".to_string(),
                vec![make_result(
                    "my_test",
                    TestStatus::Passed,
                    1000,
                    Some(TokenUsage {
                        prompt_tokens: 400,
                        completion_tokens: 200,
                    }),
                )],
            ),
            (
                "run_5".to_string(),
                vec![make_result(
                    "my_test",
                    TestStatus::Passed,
                    1000,
                    Some(TokenUsage {
                        prompt_tokens: 500,
                        completion_tokens: 250,
                    }),
                )],
            ),
        ];

        let record = analyze_stability("my_test", &runs).unwrap();
        let report = generate_stability_report(&record);
        assert!(report.contains("Token Usage Trend"));
        assert!(report.contains("Warning:"));
        assert!(report.contains("trending upward"));
    }

    #[test]
    fn test_missing_test_returns_none() {
        let runs = vec![
            (
                "run_1".to_string(),
                vec![make_result("other_test", TestStatus::Passed, 1000, None)],
            ),
            (
                "run_2".to_string(),
                vec![make_result("other_test", TestStatus::Passed, 1000, None)],
            ),
        ];
        let result = analyze_stability("my_test", &runs);
        assert!(result.is_none(), "Expected None when test not found in any run");
    }
}
