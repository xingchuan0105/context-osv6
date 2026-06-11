//! Data models for the e2e-analyzer.
//!
//! These types mirror the `meta.json` schema produced by the E2E test suite
//! in `crates/app/tests/e2e/` so the analyzer can deserialize artifacts
//! without depending on the `app` crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Core test-result types (mirror of app/tests/e2e/result_serializer.rs)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestResult {
    pub run_id: String,
    pub test_name: String,
    pub query: String,
    pub strategy: String,
    pub format_skill: Option<String>,
    pub status: TestStatus,
    pub answer_text: String,
    pub answer_html: Option<String>,
    pub screenshot_path: Option<PathBuf>,
    pub llm_calls: Vec<LlmCall>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub retrieval_hits: Option<u32>,
    pub token_usage: Option<TokenUsage>,
    pub duration_ms: u64,
    pub timestamp: String,
    pub error_message: Option<String>,
    pub diagnostics: Option<RenderDiagnostics>,
    pub failure_kind: Option<TestFailureKind>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    #[serde(alias = "passed")]
    Passed,
    #[serde(alias = "failed")]
    Failed,
    #[serde(alias = "skipped")]
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFailureKind {
    #[serde(alias = "dependency_missing")]
    DependencyMissing,
    #[serde(alias = "setup_failed")]
    SetupFailed,
    #[serde(alias = "execution_failed")]
    ExecutionFailed,
    #[serde(alias = "assertion_failed")]
    AssertionFailed,
    #[serde(alias = "cleanup_failed")]
    CleanupFailed,
    #[serde(alias = "timeout")]
    Timeout,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmCall {
    pub system_prompt: String,
    pub user_messages: Vec<serde_json::Value>,
    pub response_content: String,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolCallRecord {
    pub tool_id: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// One llm_real test artifact set under `{bucket}/{run_id}/{test_name}/`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRealTestArtifact {
    pub run_id: String,
    pub test_name: String,
    pub agent_type: Option<String>,
    pub usage: Option<serde_json::Value>,
    pub reasoning_delta_count: Option<u64>,
    pub trace_reasoning_count: Option<u64>,
    pub prompt_snapshot_count: Option<u64>,
    pub reasoning_empty_warning: Option<bool>,
    pub stream_error_with_done: Option<bool>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RenderDiagnostics {
    pub console_errors: Vec<String>,
    pub page_errors: Vec<String>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Run metadata / record types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunMetadata {
    pub run_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub git_sha: Option<String>,
    pub git_branch: Option<String>,
    pub environment: Option<serde_json::Value>,
    pub total_tests: Option<usize>,
    pub passed: Option<usize>,
    pub failed: Option<usize>,
    pub skipped: Option<usize>,
    pub timestamp: Option<String>,
    pub git_commit: Option<String>,
}

impl RunMetadata {
    /// Extract git_branch from either the top-level field or nested inside
    /// the `environment` JSON object (actual E2E artifact format).
    pub fn git_branch_from_anywhere(&self) -> Option<&str> {
        self.git_branch.as_deref().or_else(|| {
            self.environment
                .as_ref()
                .and_then(|env| env.get("git_branch").and_then(|v| v.as_str()))
        })
    }

    /// Extract git_commit from either the top-level field or nested inside
    /// the `environment` JSON object.
    pub fn git_commit_from_anywhere(&self) -> Option<&str> {
        self.git_commit.as_deref().or_else(|| {
            self.environment
                .as_ref()
                .and_then(|env| env.get("git_commit").and_then(|v| v.as_str()))
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunRecord {
    pub metadata: RunMetadata,
    pub results: Vec<TestResult>,
}

// ---------------------------------------------------------------------------
// Diff / comparison types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestFingerprint {
    pub test_name: String,
    pub strategy: String,
    pub format_skill: Option<String>,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub token_usage: Option<TokenUsage>,
    pub retrieval_hits: Option<u32>,
    pub llm_call_count: usize,
    pub tool_call_count: usize,
    pub error_message: Option<String>,
    pub failure_kind: Option<TestFailureKind>,
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiffEntry {
    pub test_name: String,
    pub dimension: DiffDimension,
    pub severity: DiffSeverity,
    pub category: DiffCategory,
    pub baseline_value: String,
    pub current_value: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffDimension {
    Status,
    Duration,
    TokenUsage,
    LlmCalls,
    ToolCalls,
    RetrievalHits,
    ErrorMessage,
    FailureKind,
    AnswerText,
    Screenshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffSeverity {
    Critical,
    Major,
    Minor,
    Info,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffCategory {
    Regression,
    Improvement,
    Flake,
    Noise,
}

// ---------------------------------------------------------------------------
// Attribution / diagnosis types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AttributionReport {
    pub test_name: String,
    pub failure_category: FailureCategory,
    pub confidence: ConfidenceLevel,
    pub suspected_layers: Vec<SuspectedLayer>,
    pub first_anomaly: Option<FirstAnomaly>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    LlmRegression,
    ToolFailure,
    RetrievalDegradation,
    RenderingIssue,
    InfrastructureFlake,
    TestAssertion,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
    Uncertain,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SuspectedLayer {
    pub layer: String,
    pub confidence: ConfidenceLevel,
    pub evidence: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FirstAnomaly {
    pub timestamp_ms: u64,
    pub description: String,
    pub llm_call_index: Option<usize>,
    pub tool_call_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Coverage types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoverageGap {
    pub test_name: String,
    pub dimension: String,
    pub priority: GapPriority,
    pub reason: String,
    pub suggested_action: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GapPriority {
    Critical,
    High,
    Medium,
    Low,
}

// ---------------------------------------------------------------------------
// Trend / stability types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StabilityRecord {
    pub test_name: String,
    pub runs: usize,
    pub pass_rate: f64,
    pub avg_duration_ms: f64,
    pub stddev_duration_ms: f64,
    pub last_status: TestStatus,
    pub category_snapshots: Vec<CategorySnapshot>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CategorySnapshot {
    pub run_id: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerfTrend {
    pub test_name: String,
    pub metric: String,
    pub values: Vec<f64>,
    pub run_ids: Vec<String>,
    pub regression: Option<PerfRegression>,
    pub drift: Option<DriftWarning>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerfRegression {
    pub threshold_pct: f64,
    pub actual_pct: f64,
    pub baseline_avg: f64,
    pub current_avg: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftWarning {
    pub window_size: usize,
    pub stddev_multiplier: f64,
    pub detected_at_run_id: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Report / summary types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonSummary {
    pub run_metadata: RunMetadata,
    pub fingerprints: Vec<TestFingerprint>,
    pub diffs: Vec<DiffEntry>,
    pub attributions: Vec<AttributionReport>,
    pub coverage_gaps: Vec<CoverageGap>,
    pub stability: Vec<StabilityRecord>,
    pub perf_trends: Vec<PerfTrend>,
    pub severity_summary: SeveritySummary,
    pub gate_status: GateStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeveritySummary {
    pub critical: usize,
    pub major: usize,
    pub minor: usize,
    pub info: usize,
}

impl SeveritySummary {
    pub fn to_gate_status(&self) -> GateStatus {
        if self.critical > 0 {
            GateStatus::Fail
        } else if self.major > 0 {
            GateStatus::Warn
        } else {
            GateStatus::Pass
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Pass,
    Fail,
    Warn,
}
