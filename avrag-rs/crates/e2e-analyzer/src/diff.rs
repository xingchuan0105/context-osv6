//! Diff engine — compare two `TestResult` values dimension by dimension.

use crate::models::{DiffCategory, DiffDimension, DiffEntry, DiffSeverity, TestResult, TestStatus};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Top-level comparison
// ---------------------------------------------------------------------------

/// Compare two test results and produce all diff entries.
pub fn compare_results(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();
    diffs.extend(compare_status(test_name, baseline, current));
    diffs.extend(compare_duration(test_name, baseline, current));
    diffs.extend(compare_token_usage(test_name, baseline, current));
    diffs.extend(compare_llm_calls(test_name, baseline, current));
    diffs.extend(compare_tool_calls(test_name, baseline, current));
    diffs.extend(compare_answer_text(test_name, baseline, current));
    diffs
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

pub fn compare_status(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    if baseline.status == current.status {
        return Vec::new();
    }

    // Skipped -> Passed is an improvement, not a regression.
    if baseline.status == TestStatus::Skipped && current.status == TestStatus::Passed {
        return Vec::new();
    }

    let (severity, category) =
        if baseline.status == TestStatus::Passed && current.status != TestStatus::Passed {
            (DiffSeverity::Critical, DiffCategory::Regression)
        } else {
            (DiffSeverity::Major, DiffCategory::Regression)
        };

    vec![DiffEntry {
        test_name: test_name.to_string(),
        dimension: DiffDimension::Status,
        severity,
        category,
        baseline_value: format!("{:?}", baseline.status),
        current_value: format!("{:?}", current.status),
        description: format!("status: {:?} -> {:?}", baseline.status, current.status),
    }]
}

// ---------------------------------------------------------------------------
// Duration
// ---------------------------------------------------------------------------

pub fn compare_duration(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    // Skip duration comparison when baseline was skipped — the baseline
    // duration is just setup/teardown time, not meaningful for comparison.
    if baseline.duration_ms == 0 || baseline.status == TestStatus::Skipped {
        return Vec::new();
    }

    let relative_change =
        (current.duration_ms as f64 - baseline.duration_ms as f64) / baseline.duration_ms as f64;

    if relative_change > 0.30 || current.duration_ms > 20_000 {
        let pct = (relative_change * 100.0).round() as i64;
        vec![DiffEntry {
            test_name: test_name.to_string(),
            dimension: DiffDimension::Duration,
            severity: DiffSeverity::Critical,
            category: DiffCategory::Regression,
            baseline_value: baseline.duration_ms.to_string(),
            current_value: current.duration_ms.to_string(),
            description: format!(
                "duration_ms: {} -> {} ({}%)",
                baseline.duration_ms, current.duration_ms, pct
            ),
        }]
    } else {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Token usage
// ---------------------------------------------------------------------------

pub fn compare_token_usage(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    let (b_tu, c_tu) = match (&baseline.token_usage, &current.token_usage) {
        (Some(b), Some(c)) => (b, c),
        _ => return Vec::new(),
    };

    let mut diffs = Vec::new();

    // Prompt tokens
    if b_tu.prompt_tokens > 0 {
        let rel =
            (c_tu.prompt_tokens as f64 - b_tu.prompt_tokens as f64) / b_tu.prompt_tokens as f64;
        let abs_delta = c_tu.prompt_tokens.saturating_sub(b_tu.prompt_tokens);
        if rel > 0.30 || abs_delta > 20_000 {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::TokenUsage,
                severity: DiffSeverity::Critical,
                category: DiffCategory::Regression,
                baseline_value: b_tu.prompt_tokens.to_string(),
                current_value: c_tu.prompt_tokens.to_string(),
                description: format!(
                    "prompt_tokens: {} -> {} (+{:.0}%)",
                    b_tu.prompt_tokens,
                    c_tu.prompt_tokens,
                    rel * 100.0
                ),
            });
        }
    }

    // Completion tokens
    if b_tu.completion_tokens > 0 {
        let rel = (c_tu.completion_tokens as f64 - b_tu.completion_tokens as f64)
            / b_tu.completion_tokens as f64;
        let abs_delta = c_tu
            .completion_tokens
            .saturating_sub(b_tu.completion_tokens);
        if rel > 0.30 || abs_delta > 10_000 {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::TokenUsage,
                severity: DiffSeverity::Critical,
                category: DiffCategory::Regression,
                baseline_value: b_tu.completion_tokens.to_string(),
                current_value: c_tu.completion_tokens.to_string(),
                description: format!(
                    "completion_tokens: {} -> {} (+{:.0}%)",
                    b_tu.completion_tokens,
                    c_tu.completion_tokens,
                    rel * 100.0
                ),
            });
        }
    }

    diffs
}

// ---------------------------------------------------------------------------
// LLM calls
// ---------------------------------------------------------------------------

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn compare_llm_calls(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // System prompt hash of first LLM call
    if let (Some(b_first), Some(c_first)) = (baseline.llm_calls.first(), current.llm_calls.first())
    {
        let b_hash = sha256_hex(&b_first.system_prompt);
        let c_hash = sha256_hex(&c_first.system_prompt);
        if b_hash != c_hash {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::LlmCalls,
                severity: DiffSeverity::Major,
                category: DiffCategory::Regression,
                baseline_value: b_hash.clone(),
                current_value: c_hash.clone(),
                description: format!(
                    "system_prompt hash changed: {}... -> {}...",
                    &b_hash[..8.min(b_hash.len())],
                    &c_hash[..8.min(c_hash.len())]
                ),
            });
        }
    }

    // Message count drift
    if baseline.llm_calls.len() > 0 {
        let b_len = baseline.llm_calls.len() as f64;
        let c_len = current.llm_calls.len() as f64;
        let delta = (c_len - b_len).abs();
        if delta / b_len > 0.20 {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::LlmCalls,
                severity: DiffSeverity::Major,
                category: DiffCategory::Regression,
                baseline_value: baseline.llm_calls.len().to_string(),
                current_value: current.llm_calls.len().to_string(),
                description: format!(
                    "llm_call count: {} -> {} ({:.0}% change)",
                    baseline.llm_calls.len(),
                    current.llm_calls.len(),
                    ((c_len - b_len) / b_len) * 100.0
                ),
            });
        }
    }

    diffs
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

pub fn compare_tool_calls(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    let b_tools: std::collections::HashSet<&str> = baseline
        .tool_calls
        .iter()
        .map(|t| t.tool_id.as_str())
        .collect();
    let c_tools: std::collections::HashSet<&str> = current
        .tool_calls
        .iter()
        .map(|t| t.tool_id.as_str())
        .collect();

    // Missing tools in current
    let missing: Vec<&str> = b_tools.difference(&c_tools).copied().collect();
    if !missing.is_empty() {
        diffs.push(DiffEntry {
            test_name: test_name.to_string(),
            dimension: DiffDimension::ToolCalls,
            severity: DiffSeverity::Critical,
            category: DiffCategory::Regression,
            baseline_value: baseline.tool_calls.len().to_string(),
            current_value: current.tool_calls.len().to_string(),
            description: format!("missing tools in current: {}", missing.join(", ")),
        });
    }

    // Count increase > 50%
    if baseline.tool_calls.len() > 0 {
        let b_len = baseline.tool_calls.len() as f64;
        let c_len = current.tool_calls.len() as f64;
        let increase = (c_len - b_len) / b_len;
        if increase > 0.50 {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::ToolCalls,
                severity: DiffSeverity::Major,
                category: DiffCategory::Regression,
                baseline_value: baseline.tool_calls.len().to_string(),
                current_value: current.tool_calls.len().to_string(),
                description: format!(
                    "tool_call count increased: {} -> {} (+{:.0}%)",
                    baseline.tool_calls.len(),
                    current.tool_calls.len(),
                    increase * 100.0
                ),
            });
        }
    }

    diffs
}

// ---------------------------------------------------------------------------
// Answer text
// ---------------------------------------------------------------------------

pub fn compare_answer_text(
    test_name: &str,
    baseline: &TestResult,
    current: &TestResult,
) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // Text length delta > 50%
    let b_len = baseline.answer_text.len() as f64;
    let c_len = current.answer_text.len() as f64;
    if b_len > 0.0 {
        let delta = (c_len - b_len).abs() / b_len;
        if delta > 0.50 {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::AnswerText,
                severity: DiffSeverity::Major,
                category: DiffCategory::Regression,
                baseline_value: baseline.answer_text.len().to_string(),
                current_value: current.answer_text.len().to_string(),
                description: format!(
                    "answer_text length: {} -> {} ({:.0}% change)",
                    baseline.answer_text.len(),
                    current.answer_text.len(),
                    delta * 100.0
                ),
            });
        }
    }

    // HTML presence changed
    match (&baseline.answer_html, &current.answer_html) {
        (Some(_), None) | (None, Some(_)) => {
            diffs.push(DiffEntry {
                test_name: test_name.to_string(),
                dimension: DiffDimension::AnswerText,
                severity: DiffSeverity::Info,
                category: DiffCategory::Noise,
                baseline_value: format!("{:?}", baseline.answer_html.is_some()),
                current_value: format!("{:?}", current.answer_html.is_some()),
                description: format!(
                    "answer_html presence changed: {} -> {}",
                    baseline.answer_html.is_some(),
                    current.answer_html.is_some()
                ),
            });
        }
        _ => {}
    }

    diffs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
    fn status_passed_to_failed_is_critical() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let current = minimal_result(TestStatus::Failed, 1000);
        let diffs = compare_status("test", &baseline, &current);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].severity, DiffSeverity::Critical);
        assert_eq!(diffs[0].category, DiffCategory::Regression);
        assert!(diffs[0].description.contains("Passed"));
        assert!(diffs[0].description.contains("Failed"));
    }

    #[test]
    fn status_skipped_to_passed_is_not_a_regression() {
        let baseline = minimal_result(TestStatus::Skipped, 1000);
        let current = minimal_result(TestStatus::Passed, 1000);
        let diffs = compare_status("test", &baseline, &current);
        assert_eq!(
            diffs.len(),
            0,
            "Skipped -> Passed should not produce a diff"
        );
    }

    #[test]
    fn duration_skipped_baseline_is_ignored() {
        let baseline = minimal_result(TestStatus::Skipped, 917);
        let mut current = minimal_result(TestStatus::Passed, 137_594);
        current.duration_ms = 137_594;
        let diffs = compare_duration("test", &baseline, &current);
        assert_eq!(
            diffs.len(),
            0,
            "Duration diff should be skipped when baseline was Skipped"
        );
    }

    #[test]
    fn duration_plus_50_pct_is_critical() {
        let baseline = minimal_result(TestStatus::Passed, 1000);
        let mut current = minimal_result(TestStatus::Passed, 1000);
        current.duration_ms = 1500; // +50%
        let diffs = compare_duration("test", &baseline, &current);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].severity, DiffSeverity::Critical);
        assert!(diffs[0].description.contains("1500"));
    }

    #[test]
    fn system_prompt_hash_change_is_major() {
        let mut baseline = minimal_result(TestStatus::Passed, 1000);
        baseline.llm_calls.push(crate::models::LlmCall {
            system_prompt: "prompt A".to_string(),
            user_messages: Vec::new(),
            response_content: String::new(),
            timestamp_ms: 0,
        });
        let mut current = minimal_result(TestStatus::Passed, 1000);
        current.llm_calls.push(crate::models::LlmCall {
            system_prompt: "prompt B".to_string(),
            user_messages: Vec::new(),
            response_content: String::new(),
            timestamp_ms: 0,
        });
        let diffs = compare_llm_calls("test", &baseline, &current);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].severity, DiffSeverity::Major);
        assert!(diffs[0].description.contains("hash changed"));
    }

    #[test]
    fn missing_tool_is_critical() {
        let mut baseline = minimal_result(TestStatus::Passed, 1000);
        baseline.tool_calls.push(crate::models::ToolCallRecord {
            tool_id: "tool_a".to_string(),
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            status: "ok".to_string(),
        });
        let current = minimal_result(TestStatus::Passed, 1000);
        let diffs = compare_tool_calls("test", &baseline, &current);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].severity, DiffSeverity::Critical);
        assert!(diffs[0].description.contains("missing tools"));
        assert!(diffs[0].description.contains("tool_a"));
    }

    #[test]
    fn html_presence_change_is_info() {
        let mut baseline = minimal_result(TestStatus::Passed, 1000);
        baseline.answer_html = Some("<p>hi</p>".to_string());
        let current = minimal_result(TestStatus::Passed, 1000);
        let diffs = compare_answer_text("test", &baseline, &current);
        let html_diff = diffs.iter().find(|d| d.description.contains("presence"));
        assert!(html_diff.is_some());
        assert_eq!(html_diff.unwrap().severity, DiffSeverity::Info);
    }
}
