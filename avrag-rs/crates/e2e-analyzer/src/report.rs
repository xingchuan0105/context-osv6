//! Report generation for the e2e-analyzer.
//!
//! Provides Markdown and JSON summary generation from diff results.

use crate::models::{
    DiffEntry, DiffSeverity, GateStatus, JsonSummary, RunMetadata, SeveritySummary, TestResult,
};
use chrono::Utc;

// ---------------------------------------------------------------------------
// Markdown report
// ---------------------------------------------------------------------------

/// Generate a Markdown regression report.
///
/// `diffs` is a map from test_name to Vec<DiffEntry> for that test.
pub fn generate_markdown_report(
    baseline_run_id: &str,
    current_run_id: &str,
    current_results: &[TestResult],
    diffs: &[(String, Vec<DiffEntry>)],
) -> String {
    let summary = summarize_diffs(diffs);

    let passed = current_results
        .iter()
        .filter(|r| matches!(r.status, crate::models::TestStatus::Passed))
        .count();
    let failed = current_results
        .iter()
        .filter(|r| matches!(r.status, crate::models::TestStatus::Failed))
        .count();
    let skipped = current_results
        .iter()
        .filter(|r| matches!(r.status, crate::models::TestStatus::Skipped))
        .count();

    let mut report = String::new();
    report.push_str("# E2E Regression Report\n\n");
    report.push_str(&format!("- **Baseline:** `{}`\n", baseline_run_id));
    report.push_str(&format!("- **Current:** `{}`\n", current_run_id));
    report.push_str(&format!(
        "- {} passed, {} failed, {} skipped\n",
        passed, failed, skipped
    ));
    report.push_str(&format!(
        "- {} critical, {} major, {} minor, {} info\n",
        summary.critical, summary.major, summary.minor, summary.info
    ));

    // Group by severity
    let severity_order = [
        (DiffSeverity::Critical, "Critical Regressions"),
        (DiffSeverity::Major, "Major Drift"),
        (DiffSeverity::Minor, "Minor Drift"),
        (DiffSeverity::Info, "Info"),
    ];

    for (severity, section_title) in &severity_order {
        let section_diffs: Vec<&(String, Vec<DiffEntry>)> = diffs
            .iter()
            .filter(|(_, entries)| entries.iter().any(|e| e.severity == *severity))
            .collect();

        if section_diffs.is_empty() {
            continue;
        }

        report.push_str(&format!("\n## {}\n", section_title));

        for (test_name, entries) in &section_diffs {
            let severity_entries: Vec<&DiffEntry> =
                entries.iter().filter(|e| e.severity == *severity).collect();

            for entry in severity_entries {
                report.push_str(&format!("\n### {}\n", test_name));
                report.push_str(&format!("- **Dimension:** {:?}\n", entry.dimension));
                report.push_str(&format!("- **Category:** {:?}\n", entry.category));
                report.push_str(&format!("- **Signal:** `{}`\n", entry.description));
                report.push_str(&format!(
                    "- **Baseline:** {}\n",
                    entry.baseline_value
                ));
                report.push_str(&format!("- **Current:** {}\n", entry.current_value));
            }
        }
    }

    report
}

// ---------------------------------------------------------------------------
// Summary helpers
// ---------------------------------------------------------------------------

/// Summarize diffs by severity.
pub fn summarize_diffs(diffs: &[(String, Vec<DiffEntry>)]) -> SeveritySummary {
    let mut summary = SeveritySummary::default();
    for (_, entries) in diffs {
        for entry in entries {
            match entry.severity {
                DiffSeverity::Critical => summary.critical += 1,
                DiffSeverity::Major => summary.major += 1,
                DiffSeverity::Minor => summary.minor += 1,
                DiffSeverity::Info => summary.info += 1,
            }
        }
    }
    summary
}

/// CI gate exit code: 1 for Fail, 0 for Warn/Pass.
pub fn exit_code(summary: &SeveritySummary) -> i32 {
    match summary.to_gate_status() {
        GateStatus::Fail => 1,
        GateStatus::Warn | GateStatus::Pass => 0,
    }
}

// ---------------------------------------------------------------------------
// JSON summary
// ---------------------------------------------------------------------------

/// Build a minimal JsonSummary from diff results.
pub fn build_json_summary(
    _baseline_run_id: &str,
    current_run_id: &str,
    diffs: &[(String, Vec<DiffEntry>)],
) -> JsonSummary {
    let severity_summary = summarize_diffs(diffs);
    let gate_status = severity_summary.to_gate_status();

    let flat_diffs: Vec<DiffEntry> = diffs
        .iter()
        .flat_map(|(_, entries)| entries.clone())
        .collect();

    JsonSummary {
        run_metadata: RunMetadata {
            run_id: current_run_id.to_string(),
            started_at: Some(Utc::now()),
            finished_at: None,
            git_sha: None,
            git_branch: None,
            environment: None,
            total_tests: None,
            passed: None,
            failed: None,
            skipped: None,
            timestamp: None,
            git_commit: None,
        },
        fingerprints: Vec::new(),
        diffs: flat_diffs,
        attributions: Vec::new(),
        coverage_gaps: Vec::new(),
        stability: Vec::new(),
        perf_trends: Vec::new(),
        severity_summary,
        gate_status,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DiffCategory, DiffDimension, DiffSeverity, TestStatus,
    };

    fn make_diff(severity: DiffSeverity) -> DiffEntry {
        DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::Status,
            severity,
            category: DiffCategory::Regression,
            baseline_value: "old".to_string(),
            current_value: "new".to_string(),
            description: "changed".to_string(),
        }
    }

    #[test]
    fn test_summarize_diffs_counts_correctly() {
        let diffs = vec![
            (
                "t1".to_string(),
                vec![
                    make_diff(DiffSeverity::Critical),
                    make_diff(DiffSeverity::Major),
                ],
            ),
            (
                "t2".to_string(),
                vec![
                    make_diff(DiffSeverity::Major),
                    make_diff(DiffSeverity::Minor),
                    make_diff(DiffSeverity::Info),
                ],
            ),
        ];
        let summary = summarize_diffs(&diffs);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.major, 2);
        assert_eq!(summary.minor, 1);
        assert_eq!(summary.info, 1);
    }

    #[test]
    fn test_exit_code_blocked_on_critical() {
        let summary = SeveritySummary {
            critical: 1,
            major: 0,
            minor: 0,
            info: 0,
        };
        assert_eq!(exit_code(&summary), 1);
    }

    #[test]
    fn test_exit_code_pass_on_major_only() {
        let summary = SeveritySummary {
            critical: 0,
            major: 3,
            minor: 1,
            info: 2,
        };
        assert_eq!(exit_code(&summary), 0);
    }

    #[test]
    fn test_markdown_report_contains_test_name() {
        let diff = DiffEntry {
            test_name: "my_test".to_string(),
            dimension: DiffDimension::LlmCalls,
            severity: DiffSeverity::Critical,
            category: DiffCategory::Regression,
            baseline_value: "1".to_string(),
            current_value: "3".to_string(),
            description: "LLM calls increased".to_string(),
        };
        let diffs = vec![("my_test".to_string(), vec![diff])];
        let current_results = vec![TestResult {
            run_id: "r1".to_string(),
            test_name: "my_test".to_string(),
            query: "q".to_string(),
            strategy: "s".to_string(),
            format_skill: None,
            status: TestStatus::Failed,
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
        }];
        let report = generate_markdown_report("base", "curr", &current_results, &diffs);
        assert!(report.contains("my_test"));
        assert!(report.contains("LLM calls increased"));
        assert!(report.contains("Critical Regressions"));
        assert!(report.contains("base"));
        assert!(report.contains("curr"));
    }
}
