//! Failure attribution engine — map Phase 1 diffs to attribution reports.
//!
//! Priority chain (first match wins):
//! 1. ToolFailure — tool error status or tool count increase
//! 2. LlmRegression — prompt hash change or LLM call count drift
//! 3. ToolFailure (missing tools) — missing tools in current
//! 4. LlmRegression (performance) — cost/perf Critical diffs
//! 5. RenderingIssue — output drift when test failed
//! 6. TestAssertion — status regression
//! 7. Unknown — fallback

use crate::models::{
    AttributionReport, ConfidenceLevel, DiffDimension, DiffEntry, FailureCategory, FirstAnomaly,
    SuspectedLayer, TestResult, TestStatus, ToolCallRecord,
};

// ---------------------------------------------------------------------------
// Top-level attribution
// ---------------------------------------------------------------------------

/// Map Phase 1 diffs to an attribution report.
///
/// Returns `None` if current passed and there are no diffs.
pub fn attribute_failures(
    test_name: &str,
    diffs: &[DiffEntry],
    baseline: &TestResult,
    current: &TestResult,
) -> Option<AttributionReport> {
    // If current passed and there are no diffs, nothing to attribute.
    if current.status == TestStatus::Passed && diffs.is_empty() {
        return None;
    }

    // 1. StateMachine / ToolFailure: current has any tool_call with status == "error"
    if let Some((idx, tool)) = find_first_tool_error(current) {
        let evidence = vec![format!("Tool '{}' returned error status", tool.tool_id)];
        let first_anomaly = Some(FirstAnomaly {
            timestamp_ms: 0,
            description: format!("Tool '{}' returned error status", tool.tool_id),
            llm_call_index: None,
            tool_call_index: Some(idx),
        });
        return Some(build_report(
            test_name,
            FailureCategory::ToolFailure,
            ConfidenceLevel::High,
            "tool_dispatch",
            evidence,
            first_anomaly,
            Vec::new(),
        ));
    }

    // 2. PromptAssembly: diff with dimension == LlmCalls and description contains "system_prompt" or "hash"
    if let Some(diff) = diffs.iter().find(|d| {
        d.dimension == DiffDimension::LlmCalls
            && (d.description.contains("system_prompt") || d.description.contains("hash"))
    }) {
        let evidence = vec![diff.description.clone()];
        return Some(build_report(
            test_name,
            FailureCategory::LlmRegression,
            ConfidenceLevel::Medium,
            "prompt_assembly",
            evidence,
            None,
            Vec::new(),
        ));
    }

    // 3. Missing tools: diff with dimension == ToolCalls and description contains "missing"
    if let Some(diff) = diffs
        .iter()
        .find(|d| d.dimension == DiffDimension::ToolCalls && d.description.contains("missing"))
    {
        let evidence = vec![diff.description.clone()];
        return Some(build_report(
            test_name,
            FailureCategory::ToolFailure,
            ConfidenceLevel::High,
            "tool_dispatch",
            evidence,
            None,
            Vec::new(),
        ));
    }

    // 4. Performance: diff with dimension == Duration or TokenUsage and severity == Critical
    if let Some(diff) = diffs.iter().find(|d| {
        (d.dimension == DiffDimension::Duration || d.dimension == DiffDimension::TokenUsage)
            && d.severity == crate::models::DiffSeverity::Critical
    }) {
        let evidence = vec![diff.description.clone()];
        return Some(build_report(
            test_name,
            FailureCategory::LlmRegression,
            ConfidenceLevel::High,
            "perf_budget",
            evidence,
            None,
            Vec::new(),
        ));
    }

    // 5. Output drift: diff with dimension == AnswerText and current status is Failed
    if current.status == TestStatus::Failed {
        if let Some(diff) = diffs
            .iter()
            .find(|d| d.dimension == DiffDimension::AnswerText)
        {
            let evidence = vec![diff.description.clone()];
            return Some(build_report(
                test_name,
                FailureCategory::RenderingIssue,
                ConfidenceLevel::Medium,
                "llm_output",
                evidence,
                None,
                Vec::new(),
            ));
        }
    }

    // 6. Status change: baseline was Passed and current is Failed
    if baseline.status == TestStatus::Passed && current.status == TestStatus::Failed {
        let first_anomaly = Some(FirstAnomaly {
            timestamp_ms: 0,
            description: "Test status regressed from Passed to Failed".to_string(),
            llm_call_index: None,
            tool_call_index: None,
        });
        return Some(build_report(
            test_name,
            FailureCategory::TestAssertion,
            ConfidenceLevel::High,
            "test_assertion",
            vec!["Status regressed from Passed to Failed".to_string()],
            first_anomaly,
            Vec::new(),
        ));
    }

    // 7. Fallback: Unknown with Low confidence
    let mut notes = Vec::new();
    if !diffs.is_empty() {
        notes.push(format!(
            "{} diffs found but no clear classification matched",
            diffs.len()
        ));
    }
    Some(build_report(
        test_name,
        FailureCategory::Unknown,
        ConfidenceLevel::Low,
        "unknown",
        diffs.iter().map(|d| d.description.clone()).collect(),
        None,
        notes,
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_first_tool_error(current: &TestResult) -> Option<(usize, &ToolCallRecord)> {
    current
        .tool_calls
        .iter()
        .enumerate()
        .find(|(_, t)| t.status.eq_ignore_ascii_case("error"))
}

fn build_report(
    test_name: &str,
    failure_category: FailureCategory,
    confidence: ConfidenceLevel,
    layer: &str,
    evidence: Vec<String>,
    first_anomaly: Option<FirstAnomaly>,
    notes: Vec<String>,
) -> AttributionReport {
    AttributionReport {
        test_name: test_name.to_string(),
        failure_category,
        confidence,
        suspected_layers: vec![SuspectedLayer {
            layer: layer.to_string(),
            confidence,
            evidence,
        }],
        first_anomaly,
        notes,
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

    fn minimal_result(status: TestStatus, duration_ms: u64) -> TestResult {
        TestResult {
            run_id: "r1".to_string(),
            test_name: "t1".to_string(),
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
            token_usage: None,
            duration_ms,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_tool_error_classified_as_tool_failure() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let mut current = minimal_result(TestStatus::Failed, 1000);
        current.tool_calls.push(ToolCallRecord {
            tool_id: "search_tool".to_string(),
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            status: "error".to_string(),
        });

        let report = attribute_failures("test", &[], &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::ToolFailure);
        assert_eq!(report.confidence, ConfidenceLevel::High);
        assert_eq!(report.suspected_layers.len(), 1);
        assert_eq!(report.suspected_layers[0].layer, "tool_dispatch");
        assert!(report
            .suspected_layers[0]
            .evidence[0]
            .contains("search_tool"));
        assert!(report.first_anomaly.is_some());
        assert_eq!(
            report.first_anomaly.as_ref().unwrap().description,
            "Tool 'search_tool' returned error status"
        );
        assert_eq!(report.first_anomaly.as_ref().unwrap().tool_call_index, Some(0));
    }

    #[test]
    fn test_no_attribution_when_passed_and_no_diffs() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Passed, 1000);

        let report = attribute_failures("test", &[], &baseline, &current);
        assert!(report.is_none());
    }

    #[test]
    fn test_status_regression_classified() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Failed, 1000);

        let report = attribute_failures("test", &[], &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::TestAssertion);
        assert_eq!(report.confidence, ConfidenceLevel::High);
        assert!(report.first_anomaly.is_some());
        assert_eq!(
            report.first_anomaly.as_ref().unwrap().description,
            "Test status regressed from Passed to Failed"
        );
    }

    #[test]
    fn test_missing_tools_classified_as_tool_failure() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Failed, 1000);

        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::ToolCalls,
            severity: DiffSeverity::Critical,
            category: DiffCategory::Regression,
            baseline_value: "1".to_string(),
            current_value: "0".to_string(),
            description: "missing tools in current: search_tool".to_string(),
        }];

        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::ToolFailure);
        assert_eq!(report.confidence, ConfidenceLevel::High);
        assert!(report.suspected_layers[0]
            .evidence[0]
            .contains("missing"));
    }

    #[test]
    fn test_prompt_hash_change_classified_as_llm_regression() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Failed, 1000);

        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::LlmCalls,
            severity: DiffSeverity::Major,
            category: DiffCategory::Regression,
            baseline_value: "abc123".to_string(),
            current_value: "def456".to_string(),
            description: "system_prompt hash changed: abc123... -> def456...".to_string(),
        }];

        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::LlmRegression);
        assert_eq!(report.confidence, ConfidenceLevel::Medium);
        assert_eq!(report.suspected_layers[0].layer, "prompt_assembly");
    }

    #[test]
    fn test_performance_critical_classified_as_llm_regression() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Failed, 1000);

        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::Duration,
            severity: DiffSeverity::Critical,
            category: DiffCategory::Regression,
            baseline_value: "1000".to_string(),
            current_value: "5000".to_string(),
            description: "duration_ms: 1000 -> 5000 (+400%)".to_string(),
        }];

        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::LlmRegression);
        assert_eq!(report.confidence, ConfidenceLevel::High);
        assert_eq!(report.suspected_layers[0].layer, "perf_budget");
    }

    #[test]
    fn test_answer_text_drift_when_failed_classified_as_rendering_issue() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let mut current = minimal_result(TestStatus::Failed, 1000);
        current.answer_text = "very different answer".to_string();

        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::AnswerText,
            severity: DiffSeverity::Major,
            category: DiffCategory::Regression,
            baseline_value: "10".to_string(),
            current_value: "50".to_string(),
            description: "answer_text length: 10 -> 50 (400% change)".to_string(),
        }];

        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::RenderingIssue);
        assert_eq!(report.confidence, ConfidenceLevel::Medium);
        assert_eq!(report.suspected_layers[0].layer, "llm_output");
    }

    #[test]
    fn test_tool_error_takes_priority_over_status_regression() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let mut current = minimal_result(TestStatus::Failed, 1000);
        current.tool_calls.push(ToolCallRecord {
            tool_id: "broken_tool".to_string(),
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            status: "ERROR".to_string(), // case-insensitive
        });

        let report = attribute_failures("test", &[], &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        // Tool error should take priority over status regression
        assert_eq!(report.failure_category, FailureCategory::ToolFailure);
        assert_eq!(report.confidence, ConfidenceLevel::High);
    }

    #[test]
    fn test_unknown_fallback_when_no_patterns_match() {
        // Status did not regress, but there are diffs that don't match any pattern
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Passed, 1000);

        // A diff that doesn't match any specific pattern
        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::RetrievalHits,
            severity: DiffSeverity::Minor,
            category: DiffCategory::Regression,
            baseline_value: "5".to_string(),
            current_value: "3".to_string(),
            description: "retrieval hits decreased".to_string(),
        }];

        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(report.failure_category, FailureCategory::Unknown);
        assert_eq!(report.confidence, ConfidenceLevel::Low);
        assert!(!report.notes.is_empty());
        assert!(report.notes[0].contains("no clear classification matched"));
    }

    #[test]
    fn test_no_attribution_when_passed_with_diffs() {
        // If current passed but there are diffs, we still attribute
        // because diffs indicate something changed
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Passed, 1000);

        let diffs = vec![DiffEntry {
            test_name: "test".to_string(),
            dimension: DiffDimension::Duration,
            severity: DiffSeverity::Major,
            category: DiffCategory::Regression,
            baseline_value: "1000".to_string(),
            current_value: "2000".to_string(),
            description: "duration increased".to_string(),
        }];

        // Current passed but has diffs - should still produce a report
        let report = attribute_failures("test", &diffs, &baseline, &current);
        assert!(report.is_some());
    }
}
