//! Test result serializer — persists E2E test artifacts and generates reports.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

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
    pub llm_calls: Vec<super::recording_llm::LlmCall>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub retrieval_hits: Option<u32>,
    pub token_usage: Option<TokenUsage>,
    pub duration_ms: u64,
    pub timestamp: String,
    pub error_message: Option<String>,
    pub diagnostics: Option<super::playwright_helper::RenderDiagnostics>,
    pub failure_kind: Option<TestFailureKind>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFailureKind {
    DependencyMissing,
    SetupFailed,
    ExecutionFailed,
    AssertionFailed,
    CleanupFailed,
    Timeout,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ArtifactRetentionPolicy {
    Never,
    OnFailure,
    Always,
}

/// Save a test result to disk under `output_dir/{test_name}/`.
pub fn save_test_result(
    output_dir: &Path,
    result: &TestResult,
    policy: ArtifactRetentionPolicy,
) -> Result<PathBuf, std::io::Error> {
    let test_dir = output_dir.join(&result.test_name);
    std::fs::create_dir_all(&test_dir)?;

    // Always save query and meta
    std::fs::write(test_dir.join("query.txt"), &result.query)?;
    std::fs::write(
        test_dir.join("meta.json"),
        serde_json::to_string_pretty(result)?,
    )?;

    let should_keep_all = match policy {
        ArtifactRetentionPolicy::Always => true,
        ArtifactRetentionPolicy::OnFailure => result.status != TestStatus::Passed,
        ArtifactRetentionPolicy::Never => false,
    };

    if should_keep_all || !result.answer_text.is_empty() {
        std::fs::write(test_dir.join("answer.txt"), &result.answer_text)?;
    }

    if let Some(ref html) = result.answer_html {
        if should_keep_all {
            std::fs::write(test_dir.join("answer.html"), html)?;
        }
    }

    if let Some(ref path) = result.screenshot_path {
        if should_keep_all && path.exists() {
            let dest = test_dir.join("screenshot.png");
            std::fs::copy(path, dest)?;
        }
    }

    if should_keep_all {
        let llm_path = test_dir.join("llm_calls.jsonl");
        let mut llm_file = std::fs::File::create(llm_path)?;
        for call in &result.llm_calls {
            serde_json::to_writer(&llm_file, call)?;
            llm_file.write_all(b"\n")?;
        }

        let tool_path = test_dir.join("tool_calls.jsonl");
        let mut tool_file = std::fs::File::create(tool_path)?;
        for call in &result.tool_calls {
            serde_json::to_writer(&tool_file, call)?;
            tool_file.write_all(b"\n")?;
        }
    }

    if let Some(ref diag) = result.diagnostics {
        if should_keep_all {
            std::fs::write(
                test_dir.join("diagnostics.json"),
                serde_json::to_string_pretty(diag)?,
            )?;
        }
    }

    Ok(test_dir)
}

/// Load all TestResults from a run directory.
pub fn load_run_results(run_dir: &Path) -> Vec<TestResult> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(run_dir) {
        Ok(e) => e,
        Err(_) => return results,
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let meta_path = entry.path().join("meta.json");
        if meta_path.exists() {
            let content = std::fs::read_to_string(meta_path).unwrap_or_default();
            if let Ok(result) = serde_json::from_str::<TestResult>(&content) {
                results.push(result);
            }
        }
    }

    results.sort_by(|a, b| a.test_name.cmp(&b.test_name));
    results
}

/// Generate a Markdown regression report from test results.
pub fn generate_markdown_report(
    run_dir: &Path,
    results: &[TestResult],
) -> Result<String, std::io::Error> {
    let mut md = String::new();
    md.push_str("# E2E Regression Report\n\n");

    let passed = results
        .iter()
        .filter(|r| r.status == TestStatus::Passed)
        .count();
    let failed = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .count();
    let skipped = results
        .iter()
        .filter(|r| r.status == TestStatus::Skipped)
        .count();

    md.push_str(&format!(
        "**Summary:** {} passed, {} failed, {} skipped\n\n",
        passed, failed, skipped
    ));

    md.push_str("## Results\n\n");
    md.push_str("| Test | Strategy | Format | Status | Duration |\n");
    md.push_str("|------|----------|--------|--------|----------|\n");

    for r in results {
        let format = r.format_skill.as_deref().unwrap_or("-");
        let status_str = match r.status {
            TestStatus::Passed => "Passed",
            TestStatus::Failed => "Failed",
            TestStatus::Skipped => "Skipped",
        };
        let status_emoji = match r.status {
            TestStatus::Passed => "✅",
            TestStatus::Failed => "❌",
            TestStatus::Skipped => "⏭️",
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} {} | {}ms |\n",
            r.test_name, r.strategy, format, status_emoji, status_str, r.duration_ms
        ));
    }

    let failures: Vec<_> = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .collect();
    if !failures.is_empty() {
        md.push_str("\n## Failures\n\n");
        for f in failures {
            md.push_str(&format!(
                "### {}\n- **Kind:** {:?}\n- **Error:** {}\n\n",
                f.test_name,
                f.failure_kind,
                f.error_message.as_deref().unwrap_or("unknown")
            ));
        }
    }

    let skips: Vec<_> = results
        .iter()
        .filter(|r| r.status == TestStatus::Skipped)
        .collect();
    if !skips.is_empty() {
        md.push_str("\n## Skipped\n\n");
        for s in skips {
            md.push_str(&format!(
                "- {}: {}\n",
                s.test_name,
                s.error_message.as_deref().unwrap_or("no reason")
            ));
        }
    }

    Ok(md)
}
