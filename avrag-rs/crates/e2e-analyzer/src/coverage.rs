//! Coverage matrix scanner — detect coverage gaps across E2E test runs.

use crate::models::{CoverageGap, GapPriority, TestResult, TestStatus};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoverageDimension {
    Strategy,
    OutputFormat,
    RiskCategory,
}

#[derive(Debug, Default)]
pub struct CoverageCell {
    pub test_names: HashSet<String>,
    pub passed_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Default)]
pub struct CoverageMatrix {
    cells: HashMap<(CoverageDimension, String), CoverageCell>,
}

impl CoverageMatrix {
    fn hit(
        &mut self,
        dim: CoverageDimension,
        value: String,
        test_name: String,
        status: TestStatus,
    ) {
        let cell = self.cells.entry((dim, value)).or_default();
        cell.test_names.insert(test_name);
        match status {
            TestStatus::Passed => cell.passed_count += 1,
            TestStatus::Failed => cell.failed_count += 1,
            TestStatus::Skipped => cell.skipped_count += 1,
        }
    }

    /// Identify coverage gaps: cells where all runs failed, or flaky rate is high.
    pub fn gaps(&self) -> Vec<CoverageGap> {
        let mut gaps = Vec::new();

        for ((dim, value), cell) in &self.cells {
            let total = cell.passed_count + cell.failed_count + cell.skipped_count;
            if total == 0 {
                continue;
            }

            // All failed → High priority gap
            if cell.passed_count == 0 && cell.failed_count > 0 {
                gaps.push(CoverageGap {
                    test_name: cell.test_names.iter().next().cloned().unwrap_or_default(),
                    dimension: format!("{:?}", dim),
                    priority: GapPriority::High,
                    reason: format!(
                        "All {} run(s) failed for {}={}",
                        total,
                        format_dim(dim),
                        value
                    ),
                    suggested_action: format!(
                        "Add or fix test covering {}={}",
                        format_dim(dim),
                        value
                    ),
                });
                continue;
            }

            // Flaky rate > 20% → Medium priority gap
            let flaky_rate = (cell.failed_count as f64) / (total as f64);
            if flaky_rate > 0.2 {
                gaps.push(CoverageGap {
                    test_name: cell.test_names.iter().next().cloned().unwrap_or_default(),
                    dimension: format!("{:?}", dim),
                    priority: GapPriority::Medium,
                    reason: format!(
                        "Flaky rate {:.0}% for {}={} ({} passed, {} failed, {} skipped over {} runs)",
                        flaky_rate * 100.0,
                        format_dim(dim),
                        value,
                        cell.passed_count,
                        cell.failed_count,
                        cell.skipped_count,
                        total
                    ),
                    suggested_action: format!(
                        "Investigate instability in {}={} coverage",
                        format_dim(dim),
                        value
                    ),
                });
            }
        }

        // Sort by risk_score descending (High > Medium > Low)
        gaps.sort_by(|a, b| {
            let score_a = gap_risk_score(a);
            let score_b = gap_risk_score(b);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        gaps
    }
}

fn gap_risk_score(gap: &CoverageGap) -> f64 {
    match gap.priority {
        GapPriority::Critical => 1.0,
        GapPriority::High => 0.9,
        GapPriority::Medium => {
            // Extract flaky rate from reason if present
            if let Some(start) = gap.reason.find("Flaky rate ") {
                let rest = &gap.reason[start + 10..];
                if let Some(end) = rest.find('%') {
                    if let Ok(pct) = rest[..end].parse::<f64>() {
                        return pct / 100.0;
                    }
                }
            }
            0.5
        }
        GapPriority::Low => 0.1,
    }
}

fn format_dim(dim: &CoverageDimension) -> &'static str {
    match dim {
        CoverageDimension::Strategy => "strategy",
        CoverageDimension::OutputFormat => "format_skill",
        CoverageDimension::RiskCategory => "risk_category",
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan multiple runs and build a coverage matrix.
pub fn build_coverage_matrix(runs: &[Vec<TestResult>]) -> CoverageMatrix {
    let mut matrix = CoverageMatrix::default();

    for run in runs {
        for result in run {
            // Strategy dimension
            matrix.hit(
                CoverageDimension::Strategy,
                result.strategy.clone(),
                result.test_name.clone(),
                result.status,
            );

            // OutputFormat dimension
            if let Some(ref format) = result.format_skill {
                matrix.hit(
                    CoverageDimension::OutputFormat,
                    format.clone(),
                    result.test_name.clone(),
                    result.status,
                );
            }

            // RiskCategory dimension
            let risk = infer_risk_category(&result.test_name);
            matrix.hit(
                CoverageDimension::RiskCategory,
                risk,
                result.test_name.clone(),
                result.status,
            );
        }
    }

    matrix
}

/// Infer risk category from test name.
pub fn infer_risk_category(test_name: &str) -> String {
    let lower = test_name.to_lowercase();
    if lower.contains("inject") || lower.contains("guard") || lower.contains("jailbreak") {
        "injection".to_string()
    } else if lower.contains("empty") || lower.contains("budget") {
        "empty_input".to_string()
    } else if lower.contains("cancel") {
        "cancellation".to_string()
    } else if lower.contains("format") || lower.contains("ppt") || lower.contains("html") {
        "format_constraint".to_string()
    } else {
        "general".to_string()
    }
}

/// Generate a Markdown coverage report from gaps.
pub fn generate_coverage_report(gaps: &[CoverageGap]) -> String {
    let mut report = String::new();
    report.push_str("# Coverage Gap Report\n\n");

    if gaps.is_empty() {
        report.push_str("No coverage gaps detected.\n");
        return report;
    }

    report.push_str(&format!("{} gap(s) detected.\n\n", gaps.len()));
    report.push_str("| Priority | Risk Score | Dimension | Evidence | Recommended Pattern |\n");
    report.push_str("|----------|------------|-----------|----------|---------------------|\n");

    for gap in gaps {
        let risk_score = gap_risk_score(gap);
        let priority = format!("{:?}", gap.priority);
        let dim = &gap.dimension;
        let evidence = &gap.reason;
        let recommended = &gap.suggested_action;
        report.push_str(&format!(
            "| {} | {:.2} | {} | {} | {} |\n",
            priority, risk_score, dim, evidence, recommended
        ));
    }

    report
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(
        test_name: &str,
        strategy: &str,
        format_skill: Option<&str>,
        status: TestStatus,
    ) -> TestResult {
        TestResult {
            run_id: "r1".to_string(),
            test_name: test_name.to_string(),
            query: "q".to_string(),
            strategy: strategy.to_string(),
            format_skill: format_skill.map(String::from),
            status,
            answer_text: String::new(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: Vec::new(),
            tool_calls: Vec::new(),
            retrieval_hits: None,
            token_usage: None,
            duration_ms: 1000,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_coverage_detects_all_failing() {
        // 2 runs, same test failing both times → High gap
        let run1 = vec![make_result("test_a", "chat", None, TestStatus::Failed)];
        let run2 = vec![make_result("test_a", "chat", None, TestStatus::Failed)];
        let matrix = build_coverage_matrix(&[run1, run2]);
        let gaps = matrix.gaps();

        // Should find at least the Strategy=chat gap (all failing)
        let high_gaps: Vec<_> = gaps
            .iter()
            .filter(|g| g.priority == GapPriority::High)
            .collect();
        assert!(
            !high_gaps.is_empty(),
            "Expected at least one High priority gap for all-failing cell, got gaps: {:?}",
            gaps
        );

        let strategy_gap = high_gaps.iter().find(|g| g.dimension.contains("Strategy"));
        assert!(strategy_gap.is_some(), "Expected a Strategy dimension gap");
    }

    #[test]
    fn test_coverage_detects_flaky() {
        // 5 runs: P, F, P, F, F → flaky_rate = 0.6 → Medium gap
        let runs: Vec<Vec<TestResult>> = vec![
            vec![make_result("test_b", "rag", None, TestStatus::Passed)],
            vec![make_result("test_b", "rag", None, TestStatus::Failed)],
            vec![make_result("test_b", "rag", None, TestStatus::Passed)],
            vec![make_result("test_b", "rag", None, TestStatus::Failed)],
            vec![make_result("test_b", "rag", None, TestStatus::Failed)],
        ];
        let matrix = build_coverage_matrix(&runs);
        let gaps = matrix.gaps();

        let medium_gaps: Vec<_> = gaps
            .iter()
            .filter(|g| g.priority == GapPriority::Medium)
            .collect();
        assert!(
            !medium_gaps.is_empty(),
            "Expected at least one Medium priority gap for flaky cell, got gaps: {:?}",
            gaps
        );
    }

    #[test]
    fn test_infer_risk_category() {
        assert_eq!(infer_risk_category("prompt_injection_test"), "injection");
        assert_eq!(infer_risk_category("guard_rail_check"), "injection");
        assert_eq!(infer_risk_category("jailbreak_attempt"), "injection");
        assert_eq!(infer_risk_category("empty_input_handling"), "empty_input");
        assert_eq!(infer_risk_category("budget_exceeded"), "empty_input");
        assert_eq!(infer_risk_category("cancel_operation"), "cancellation");
        assert_eq!(
            infer_risk_category("format_output_test"),
            "format_constraint"
        );
        assert_eq!(infer_risk_category("ppt_generation"), "format_constraint");
        assert_eq!(infer_risk_category("html_rendering"), "format_constraint");
        assert_eq!(infer_risk_category("chat_simple"), "general");
    }

    #[test]
    fn test_coverage_report_format() {
        let gaps = vec![CoverageGap {
            test_name: "t".to_string(),
            dimension: "Strategy".to_string(),
            priority: GapPriority::High,
            reason: "All runs failed".to_string(),
            suggested_action: "Fix it".to_string(),
        }];
        let report = generate_coverage_report(&gaps);
        assert!(report.contains("# Coverage Gap Report"));
        assert!(report.contains("Priority"));
        assert!(report.contains("Risk Score"));
        assert!(report.contains("Dimension"));
        assert!(report.contains("Evidence"));
        assert!(report.contains("Recommended Pattern"));
        assert!(report.contains("High"));
        assert!(report.contains("0.90"));
    }

    #[test]
    fn test_coverage_report_empty() {
        let report = generate_coverage_report(&[]);
        assert!(report.contains("No coverage gaps detected"));
    }

    #[test]
    fn test_format_skill_dimension_tracked() {
        let run = vec![make_result(
            "test_c",
            "chat",
            Some("ppt"),
            TestStatus::Failed,
        )];
        let matrix = build_coverage_matrix(&[run]);
        let gaps = matrix.gaps();
        let format_gap = gaps.iter().find(|g| g.dimension.contains("OutputFormat"));
        assert!(format_gap.is_some(), "Expected OutputFormat dimension gap");
    }
}
