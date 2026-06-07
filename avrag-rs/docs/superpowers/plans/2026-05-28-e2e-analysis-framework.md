# E2E Analysis Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI (`e2e-analyzer`) that consumes `e2e_output/{run_id}/` artifacts and produces cross-run regression reports, failure attribution diagnosis, coverage governance matrices, and stability trend analysis.

**Architecture:** Layered analyzer in a standalone crate (`crates/e2e-analyzer`). Each phase is a separate module: `diff` (Phase 1), `attribution` (Phase 2), `coverage` (Phase 3), `stability` (Phase 4). A shared `models` module defines all data types. `loader` reads artifact directories. `report` renders Markdown + JSON output. `baseline` manages persistent baseline references. CLI uses `clap` with subcommands: `diff`, `diagnose`, `coverage`, `trends`, `report`, `baseline`.

**Tech Stack:** Rust, clap, serde_json, sha2, walkdir.

---

## File Structure

```
crates/e2e-analyzer/
├── Cargo.toml
└── src/
    ├── main.rs           # CLI entry point, subcommand dispatch
    ├── cli.rs            # clap argument definitions (all subcommands)
    ├── models.rs         # Data types: TestResult, RunRecord, DiffEntry, AttributionReport, CoverageGap, StabilityRecord, enums
    ├── loader.rs         # Read e2e_output directories, parse meta.json/metadata.json, load runs
    ├── baseline.rs       # .e2e_baseline file read/write, baseline selection fallback logic
    ├── fingerprint.rs    # SHA-256 hash of test source function bodies
    ├── diff.rs           # Phase 1: four-dimension diff engine (Prompt, Behavior, Output, CostPerf)
    ├── attribution.rs    # Phase 2: failure attribution with first-anomaly localization
    ├── coverage.rs       # Phase 3: coverage matrix scanner, risk scoring, gap detection (P1)
    ├── stability.rs      # Phase 4: flaky detection, performance trend analysis (P2)
    └── report.rs         # Markdown + JSON report generation, CI gate exit codes
```

**Design notes:**
- `models` defines all types that mirror the existing `TestResult` from `app/tests/e2e/result_serializer.rs` but lives independently so the analyzer does not depend on `app` crate test code.
- `loader` reads from `crates/app/tests/e2e_output/` by default (configurable via `--output-dir`).
- Each diff dimension is a pure function `fn compare_*(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry>` for testability.
- Attribution maps from Phase 1 diffs via a priority-ordered match table (spec section 5.5).

---

## Milestone 1: Phase 1 + Phase 2 (P0)

### Task 1: Create crate and data models

**Files:**
- Create: `crates/e2e-analyzer/Cargo.toml`
- Create: `crates/e2e-analyzer/src/main.rs`
- Create: `crates/e2e-analyzer/src/cli.rs`
- Create: `crates/e2e-analyzer/src/models.rs`

- [ ] **Step 1: Add crate to workspace**

Add `"crates/e2e-analyzer"` to the workspace `members` array in `/home/chuan/context-osv6/avrag-rs/Cargo.toml`.

```toml
# In Cargo.toml, find the members list and add:
"crates/e2e-analyzer",
```

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "e2e-analyzer"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true

[[bin]]
name = "e2e-analyzer"
path = "src/main.rs"

[dependencies]
clap = { version = "4.5", features = ["derive"] }
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true
chrono.workspace = true
walkdir = "2.5"
anyhow.workspace = true
```

- [ ] **Step 3: Create cli.rs**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "e2e-analyzer")]
#[command(about = "Cross-run E2E test analysis framework")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Base directory containing e2e_output/ runs.
    #[arg(long, global = true, default_value = "crates/app/tests/e2e_output")]
    pub output_dir: PathBuf,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Phase 1: compare current run against baseline.
    Diff {
        #[arg(long)]
        baseline_run_id: Option<String>,
        #[arg(long)]
        current_run_id: String,
    },
    /// Phase 2: diagnose failures in a run (auto-runs diff if needed).
    Diagnose {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        baseline_run_id: Option<String>,
    },
    /// Phase 3: coverage matrix over last N runs.
    Coverage {
        #[arg(long, default_value = "30")]
        runs: usize,
    },
    /// Phase 4: stability trends for a specific test.
    Trends {
        #[arg(long)]
        test_name: String,
        #[arg(long, default_value = "20")]
        runs: usize,
    },
    /// Combined report (runs all applicable phases).
    Report {
        #[arg(long)]
        current_run_id: String,
        #[arg(long)]
        baseline_run_id: Option<String>,
    },
    /// Baseline management.
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },
}

#[derive(Subcommand)]
pub enum BaselineAction {
    /// Promote a run to be the persistent baseline.
    Promote { run_id: String },
    /// Show current baseline.
    Show,
}
```

- [ ] **Step 4: Create models.rs**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Compatibility types — mirror app/tests/e2e/result_serializer.rs
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
    pub screenshot_path: Option<std::path::PathBuf>,
    pub llm_calls: Vec<LlmCall>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub retrieval_hits: Option<u32>,
    pub token_usage: Option<TokenUsage>,
    pub duration_ms: u64,
    pub timestamp: String,
    pub error_message: Option<String>,
    pub diagnostics: Option<serde_json::Value>,
    pub failure_kind: Option<TestFailureKind>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestFailureKind {
    DependencyMissing,
    SetupFailed,
    ExecutionFailed,
    AssertionFailed,
    CleanupFailed,
    Timeout,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmCall {
    pub system_prompt: String,
    #[serde(default)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunMetadata {
    pub run_id: String,
    pub timestamp: String,
    pub environment: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Analyzer data model
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub branch: String,
    pub commit: String,
    pub timestamp: String,
    pub results: Vec<TestResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestFingerprint {
    pub test_name: String,
    pub source_file: String,
    pub source_hash: String,
    pub case_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiffEntry {
    pub dimension: DiffDimension,
    pub severity: DiffSeverity,
    pub category: DiffCategory,
    pub raw_diff: String,
    pub normalized_signal: String,
    pub baseline_value: serde_json::Value,
    pub current_value: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffDimension {
    Prompt,
    Behavior,
    Output,
    CostPerf,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffSeverity {
    Hard,
    Soft,
    Info,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffCategory {
    Functional,
    NonFunctional,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AttributionReport {
    pub test_name: String,
    pub fingerprint_match: bool,
    pub category: FailureCategory,
    pub severity: DiffSeverity,
    pub confidence: ConfidenceLevel,
    pub suspected_layers: Vec<SuspectedLayer>,
    pub first_anomaly: Option<FirstAnomaly>,
    pub related_diffs: Vec<DiffEntry>,
    pub suggested_action: String,
    pub diagnostic_notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    StateMachineFailure,
    PromptAssemblyFailure,
    ToolExecutionFailure,
    ModelBehaviorFailure,
    PerformanceRegression,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuspectedLayer {
    Fsm,
    PromptAssembly,
    ToolDispatch,
    LlmOutput,
    PerfBudget,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FirstAnomaly {
    pub stage: String,
    pub iteration: u32,
    pub expected_next: Vec<String>,
    pub actual_next: String,
    pub reasoning: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoverageGap {
    pub priority: GapPriority,
    pub risk_score: f32,
    pub dimensions: HashMap<String, String>,
    pub related_tests: Vec<String>,
    pub evidence: String,
    pub recommended_test_pattern: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GapPriority {
    High,
    Medium,
    Info,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StabilityRecord {
    pub test_name: String,
    pub fingerprint_hash: String,
    pub flaky_rate: f32,
    pub runs_analyzed: u32,
    pub consecutive_failures: u32,
    pub category_history: Vec<CategorySnapshot>,
    pub perf_trend: PerfTrend,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CategorySnapshot {
    pub run_id: String,
    pub category: Option<FailureCategory>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PerfTrend {
    pub hard_regressions: Vec<PerfRegression>,
    pub drift_warnings: Vec<DriftWarning>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerfRegression {
    pub run_id: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftWarning {
    pub metric: String,
    pub slope: f64,
    pub runs_window: usize,
    pub values: Vec<f64>,
}

// ---------------------------------------------------------------------------
// Report output types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonSummary {
    pub baseline_run_id: String,
    pub current_run_id: String,
    pub summary: SeveritySummary,
    pub gate_status: GateStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeveritySummary {
    pub hard: usize,
    pub soft: usize,
    pub info: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Blocked,
    ReviewRequired,
    Pass,
}

impl SeveritySummary {
    pub fn to_gate_status(&self) -> GateStatus {
        if self.hard > 0 {
            GateStatus::Blocked
        } else if self.soft > 0 {
            GateStatus::ReviewRequired
        } else {
            GateStatus::Pass
        }
    }
}
```

- [ ] **Step 5: Create main.rs scaffold**

```rust
mod cli;
mod models;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    println!("e2e-analyzer: {:?}", cli.command);
}
```

- [ ] **Step 6: Verify crate compiles**

Run: `cargo check -p e2e-analyzer`
Expected: clean compile, no errors.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): create crate with data models and CLI scaffold"
```

---

### Task 2: Artifact loader

**Files:**
- Create: `crates/e2e-analyzer/src/loader.rs`
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Write loader.rs**

```rust
use crate::models::{RunMetadata, TestResult};
use std::path::{Path, PathBuf};

/// Load all TestResults from a single run directory.
/// Expects: `{run_dir}/{test_name}/meta.json`
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
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        match serde_json::from_str::<TestResult>(&content) {
            Ok(mut result) => {
                // Load LLM calls from llm_calls.jsonl if present
                let llm_path = path.join("llm_calls.jsonl");
                if llm_path.exists() {
                    if let Ok(calls) = load_llm_calls(&llm_path) {
                        result.llm_calls = calls;
                    }
                }
                // Load tool calls from tool_calls.jsonl if present
                let tool_path = path.join("tool_calls.jsonl");
                if tool_path.exists() {
                    if let Ok(calls) = load_tool_calls(&tool_path) {
                        result.tool_calls = calls;
                    }
                }
                results.push(result);
            }
            Err(e) => {
                eprintln!("Failed to parse {}: {}", meta_path.display(), e);
            }
        }
    }

    results.sort_by(|a, b| a.test_name.cmp(&b.test_name));
    results
}

/// Load metadata.json for a run directory.
pub fn load_run_metadata(run_dir: &Path) -> Option<RunMetadata> {
    let path = run_dir.join("metadata.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Discover all run directories under the output base directory.
/// Returns sorted list (newest last) based on directory modification time.
pub fn discover_runs(output_dir: &Path) -> Vec<PathBuf> {
    let mut runs = Vec::new();
    let entries = match std::fs::read_dir(output_dir) {
        Ok(e) => e,
        Err(_) => return runs,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("e2e_") && entry.path().is_dir() {
            runs.push(entry.path());
        }
    }

    runs.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    runs
}

/// Find a specific run directory by run_id.
pub fn find_run_dir(output_dir: &Path, run_id: &str) -> Option<PathBuf> {
    discover_runs(output_dir)
        .into_iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == run_id)
                .unwrap_or(false)
        })
}

/// Find the latest successful run on a given branch.
pub fn find_latest_run_on_branch(output_dir: &Path, branch: &str) -> Option<PathBuf> {
    let runs = discover_runs(output_dir);
    for run_dir in runs.iter().rev() {
        let metadata = load_run_metadata(run_dir);
        let env_branch = metadata.as_ref().and_then(|m| {
            m.environment
                .as_ref()
                .and_then(|e| e.get("git_branch"))
                .and_then(|b| b.as_str())
        });
        if env_branch == Some(branch) {
            let results = load_run_results(run_dir);
            let has_passed = results.iter().any(|r| matches!(r.status, crate::models::TestStatus::Passed));
            if has_passed {
                return Some(run_dir.clone());
            }
        }
    }
    None
}

fn load_llm_calls(path: &Path) -> Result<Vec<crate::models::LlmCall>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let mut calls = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(call) = serde_json::from_str::<crate::models::LlmCall>(line) {
            calls.push(call);
        }
    }
    Ok(calls)
}

fn load_tool_calls(path: &Path) -> Result<Vec<crate::models::ToolCallRecord>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let mut calls = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(call) = serde_json::from_str::<crate::models::ToolCallRecord>(line) {
            calls.push(call);
        }
    }
    Ok(calls)
}
```

- [ ] **Step 2: Add loader module to main.rs**

```rust
mod cli;
mod loader;
mod models;
```

- [ ] **Step 3: Write test for loader**

Create `crates/e2e-analyzer/src/loader.rs` and add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TestStatus;
    use std::io::Write;

    #[test]
    fn test_load_run_results_parses_meta_json() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = tmp.path().join("e2e_20260528-000000_aaaaaaaa");
        let test_dir = run_dir.join("test_chat_simple");
        std::fs::create_dir_all(&test_dir).unwrap();

        let meta = serde_json::json!({
            "run_id": "e2e_20260528-000000_aaaaaaaa",
            "test_name": "test_chat_simple",
            "query": "hello",
            "strategy": "Chat",
            "format_skill": null,
            "status": "passed",
            "answer_text": "hi",
            "answer_html": null,
            "screenshot_path": null,
            "llm_calls": [],
            "tool_calls": [],
            "retrieval_hits": null,
            "token_usage": null,
            "duration_ms": 1000,
            "timestamp": "2026-05-28T00:00:00Z",
            "error_message": null,
            "diagnostics": null,
            "failure_kind": null,
        });
        std::fs::write(test_dir.join("meta.json"), meta.to_string()).unwrap();

        let results = load_run_results(&run_dir);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].test_name, "test_chat_simple");
        assert!(matches!(results[0].status, TestStatus::Passed));
    }
}
```

Note: Add `tempfile = "3"` to dev-dependencies in `crates/e2e-analyzer/Cargo.toml`.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p e2e-analyzer`
Expected: loader tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add artifact loader with run discovery"
```

---

### Task 3: Baseline management

**Files:**
- Create: `crates/e2e-analyzer/src/baseline.rs`
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Write baseline.rs**

```rust
use std::path::{Path, PathBuf};

const BASELINE_FILE: &str = ".e2e_baseline";

/// Read the persistent baseline run_id from `.e2e_baseline` file.
pub fn read_persistent_baseline(output_dir: &Path) -> Option<String> {
    let path = output_dir.join(BASELINE_FILE);
    let content = std::fs::read_to_string(&path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Write a run_id as the persistent baseline.
pub fn write_persistent_baseline(output_dir: &Path, run_id: &str) -> std::io::Result<()> {
    let path = output_dir.join(BASELINE_FILE);
    std::fs::write(&path, run_id)
}

/// Resolve baseline run directory using three-tier fallback:
/// 1. `.e2e_baseline` file
/// 2. `--baseline-run-id` CLI flag
/// 3. latest successful run on same branch as current run
pub fn resolve_baseline(
    output_dir: &Path,
    cli_baseline: Option<&str>,
    current_run_dir: &Path,
) -> Option<PathBuf> {
    // Tier 1: persistent baseline file
    if let Some(run_id) = read_persistent_baseline(output_dir) {
        if let Some(dir) = crate::loader::find_run_dir(output_dir, &run_id) {
            return Some(dir);
        }
    }

    // Tier 2: CLI flag
    if let Some(run_id) = cli_baseline {
        if let Some(dir) = crate::loader::find_run_dir(output_dir, run_id) {
            return Some(dir);
        }
    }

    // Tier 3: latest successful run on same branch
    let current_meta = crate::loader::load_run_metadata(current_run_dir);
    let branch = current_meta.as_ref().and_then(|m| {
        m.environment
            .as_ref()
            .and_then(|e| e.get("git_branch"))
            .and_then(|b| b.as_str())
    });
    if let Some(branch) = branch {
        if let Some(dir) = crate::loader::find_latest_run_on_branch(output_dir, branch) {
            // Don't use the current run as its own baseline
            if dir != current_run_dir {
                return Some(dir);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_baseline_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();
        assert_eq!(read_persistent_baseline(output_dir), None);

        write_persistent_baseline(output_dir, "e2e_20260528-000000_aaaaaaaa").unwrap();
        assert_eq!(
            read_persistent_baseline(output_dir),
            Some("e2e_20260528-000000_aaaaaaaa".to_string())
        );
    }
}
```

- [ ] **Step 2: Add baseline module to main.rs**

```rust
mod baseline;
mod cli;
mod loader;
mod models;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p e2e-analyzer baseline`
Expected: baseline tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add baseline management with three-tier fallback"
```

---

### Task 4: Test fingerprinting

**Files:**
- Create: `crates/e2e-analyzer/src/fingerprint.rs`

- [ ] **Step 1: Write fingerprint.rs**

```rust
use sha2::{Digest, Sha256};
use std::path::Path;

/// Compute SHA-256 hash of a test function's source body.
/// For now, hashes the entire source file — future: use syn to extract function body.
pub fn compute_source_hash(source_path: &Path) -> String {
    let content = std::fs::read_to_string(source_path).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check if two fingerprints match (source code unchanged).
pub fn fingerprint_match(a: &str, b: &str) -> bool {
    a == b && !a.is_empty()
}

/// Build a fingerprint for a test by name.
/// Looks up the test in known E2E test files.
pub fn fingerprint_for_test(test_name: &str) -> Option<TestFingerprint> {
    // Known test files and their mapping to test names
    let candidates = [
        (
            "crates/app/tests/e2e_chat.rs",
            vec![
                "chat_simple_conversation_state_machine",
                "chat_with_tool_call_state_machine",
                "chat_ppt_format_skill_injected",
                "chat_content_guard_redacts_injection",
                "chat_conversation_history_load_end_to_end",
                "chat_conversation_history_tools_in_catalog",
            ],
        ),
        (
            "crates/app/tests/e2e_rag.rs",
            vec![
                "rag_single_pass",
                "rag_replan_evaluate_loop",
                "rag_html_format_skill_injected",
                "rag_content_guard_redacts_injection",
                "rag_empty_document_degrades_gracefully",
            ],
        ),
        (
            "crates/app/tests/e2e_search.rs",
            vec![
                "search_single_pass",
                "search_vertical_escalation",
                "search_budget_exhaustion_degrades",
                "search_cancellation_terminates_gracefully",
                "search_content_guard_redacts_injection",
            ],
        ),
        (
            "crates/app/tests/e2e_format_output.rs",
            vec!["format_output_golden_scenarios"],
        ),
        (
            "crates/app/tests/e2e_ingestion_answer.rs",
            vec!["ingestion_answer_end_to_end"],
        ),
    ];

    for (file, tests) in &candidates {
        if tests.contains(&test_name) {
            let path = Path::new(file);
            let hash = compute_source_hash(path);
            return Some(TestFingerprint {
                test_name: test_name.to_string(),
                source_file: file.to_string(),
                source_hash: hash,
                case_version: "1.0".to_string(),
            });
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct TestFingerprint {
    pub test_name: String,
    pub source_file: String,
    pub source_hash: String,
    pub case_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_source_hash_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.rs");
        std::fs::write(&path, "fn main() {}").unwrap();

        let h1 = compute_source_hash(&path);
        let h2 = compute_source_hash(&path);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_fingerprint_match() {
        assert!(fingerprint_match("abc", "abc"));
        assert!(!fingerprint_match("abc", "def"));
        assert!(!fingerprint_match("", ""));
    }
}
```

- [ ] **Step 2: Add fingerprint module to main.rs**

```rust
mod baseline;
mod cli;
mod fingerprint;
mod loader;
mod models;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p e2e-analyzer fingerprint`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add test fingerprinting with source hash"
```

---

### Task 5: Prompt Diff dimension

**Files:**
- Create: `crates/e2e-analyzer/src/diff.rs`
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Write diff.rs — prompt dimension**

```rust
use crate::models::{DiffCategory, DiffDimension, DiffEntry, DiffSeverity, TestResult};

/// Compare two runs and produce diff entries across all four dimensions.
pub fn compare_runs(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();
    diffs.extend(compare_prompt(baseline, current));
    diffs.extend(compare_behavior(baseline, current));
    diffs.extend(compare_output(baseline, current));
    diffs.extend(compare_cost_perf(baseline, current));
    diffs
}

// ---------------------------------------------------------------------------
// Prompt Diff (Functional)
// ---------------------------------------------------------------------------

pub fn compare_prompt(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // system prompt hash: compare first LLM call's system_prompt
    if let (Some(b_call), Some(c_call)) = (baseline.llm_calls.first(), current.llm_calls.first()) {
        let b_hash = sha256_hex(&b_call.system_prompt);
        let c_hash = sha256_hex(&c_call.system_prompt);
        if b_hash != c_hash {
            diffs.push(DiffEntry {
                dimension: DiffDimension::Prompt,
                severity: DiffSeverity::Info,
                category: DiffCategory::Functional,
                raw_diff: format!("system_prompt_hash: {} -> {}", b_hash, c_hash),
                normalized_signal: format!("system_prompt_hash_changed: {} -> {}", &b_hash[..16], &c_hash[..16]),
                baseline_value: serde_json::json!({"hash": b_hash}),
                current_value: serde_json::json!({"hash": c_hash}),
            });
        }
    }

    // message count: number of LLM calls
    let b_msg_count = baseline.llm_calls.len();
    let c_msg_count = current.llm_calls.len();
    if b_msg_count > 0 {
        let delta_pct = ((c_msg_count as f64 - b_msg_count as f64) / b_msg_count as f64 * 100.0).abs();
        if delta_pct > 20.0 {
            diffs.push(DiffEntry {
                dimension: DiffDimension::Prompt,
                severity: DiffSeverity::Soft,
                category: DiffCategory::Functional,
                raw_diff: format!("llm_call_count: {} -> {}", b_msg_count, c_msg_count),
                normalized_signal: format!("llm_call_count: {} -> {} ({:.0}%)", b_msg_count, c_msg_count, delta_pct),
                baseline_value: serde_json::json!(b_msg_count),
                current_value: serde_json::json!(c_msg_count),
            });
        }
    }

    // tool catalog set: compare tool_calls tool_id sets
    let b_tools: std::collections::HashSet<_> = baseline.tool_calls.iter().map(|t| &t.tool_id).collect();
    let c_tools: std::collections::HashSet<_> = current.tool_calls.iter().map(|t| &t.tool_id).collect();
    let missing_in_current: Vec<_> = b_tools.difference(&c_tools).cloned().cloned().collect();
    if !missing_in_current.is_empty() {
        diffs.push(DiffEntry {
            dimension: DiffDimension::Prompt,
            severity: DiffSeverity::Hard,
            category: DiffCategory::Functional,
            raw_diff: format!("missing_tools_in_current: {:?}", missing_in_current),
            normalized_signal: format!("tool_catalog_count: {} -> {}", b_tools.len(), c_tools.len()),
            baseline_value: serde_json::json!(b_tools.iter().cloned().collect::<Vec<_>>()),
            current_value: serde_json::json!(c_tools.iter().cloned().collect::<Vec<_>>()),
        });
    }

    diffs
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod prompt_tests {
    use super::*;
    use crate::models::{LlmCall, TestStatus};

    fn dummy_result() -> TestResult {
        TestResult {
            run_id: "r1".to_string(),
            test_name: "t1".to_string(),
            query: "q".to_string(),
            strategy: "Chat".to_string(),
            format_skill: None,
            status: TestStatus::Passed,
            answer_text: "a".to_string(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: vec![],
            tool_calls: vec![],
            retrieval_hits: None,
            token_usage: None,
            duration_ms: 1000,
            timestamp: "2026-05-28T00:00:00Z".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_prompt_system_prompt_hash_change() {
        let mut b = dummy_result();
        b.llm_calls.push(LlmCall {
            system_prompt: "system A".to_string(),
            user_messages: vec![],
            response_content: "ok".to_string(),
            timestamp_ms: 0,
        });
        let mut c = dummy_result();
        c.llm_calls.push(LlmCall {
            system_prompt: "system B".to_string(),
            user_messages: vec![],
            response_content: "ok".to_string(),
            timestamp_ms: 0,
        });

        let diffs = compare_prompt(&b, &c);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].severity, DiffSeverity::Info));
    }

    #[test]
    fn test_prompt_tool_catalog_missing() {
        let mut b = dummy_result();
        b.tool_calls.push(crate::models::ToolCallRecord {
            tool_id: "calculator".to_string(),
            input: serde_json::json!({}),
            output: serde_json::json!({}),
            status: "ok".to_string(),
        });
        let c = dummy_result();

        let diffs = compare_prompt(&b, &c);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].severity, DiffSeverity::Hard));
    }
}
```

- [ ] **Step 2: Add diff module to main.rs**

```rust
mod baseline;
mod cli;
mod diff;
mod fingerprint;
mod loader;
mod models;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p e2e-analyzer prompt`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add prompt diff dimension"
```

---

### Task 6: Behavior and Output Diff dimensions

**Files:**
- Modify: `crates/e2e-analyzer/src/diff.rs`

- [ ] **Step 1: Add behavior diff**

Append to `diff.rs`:

```rust
// ---------------------------------------------------------------------------
// Behavior Diff (Functional)
// ---------------------------------------------------------------------------

pub fn compare_behavior(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // final decision/status change
    let b_status = format!("{:?}", baseline.status);
    let c_status = format!("{:?}", current.status);
    if b_status != c_status {
        let severity = if baseline.status == TestStatus::Passed && current.status != TestStatus::Passed {
            DiffSeverity::Hard
        } else {
            DiffSeverity::Soft
        };
        diffs.push(DiffEntry {
            dimension: DiffDimension::Behavior,
            severity,
            category: DiffCategory::Functional,
            raw_diff: format!("status: {} -> {}", b_status, c_status),
            normalized_signal: format!("status: {} -> {}", b_status, c_status),
            baseline_value: serde_json::json!(b_status),
            current_value: serde_json::json!(c_status),
        });
    }

    // replan count: inferred from tool_calls with replan-like decisions
    // (simplified: count decisions recorded in meta — not available directly,
    //  so we use tool call count as proxy for complexity change)
    let b_tool_count = baseline.tool_calls.len();
    let c_tool_count = current.tool_calls.len();
    if b_tool_count > 0 {
        let delta_pct = ((c_tool_count as f64 - b_tool_count as f64) / b_tool_count as f64 * 100.0).abs();
        if delta_pct > 50.0 && c_tool_count > b_tool_count {
            diffs.push(DiffEntry {
                dimension: DiffDimension::Behavior,
                severity: DiffSeverity::Soft,
                category: DiffCategory::Functional,
                raw_diff: format!("tool_call_count: {} -> {}", b_tool_count, c_tool_count),
                normalized_signal: format!("tool_call_count: {} -> {} (+{:.0}%)", b_tool_count, c_tool_count, delta_pct),
                baseline_value: serde_json::json!(b_tool_count),
                current_value: serde_json::json!(c_tool_count),
            });
        }
    }

    diffs
}
```

- [ ] **Step 2: Add output diff**

Append to `diff.rs`:

```rust
// ---------------------------------------------------------------------------
// Output Diff (Functional)
// ---------------------------------------------------------------------------

pub fn compare_output(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // text length delta
    let b_len = baseline.answer_text.len();
    let c_len = current.answer_text.len();
    if b_len > 0 {
        let delta_pct = ((c_len as f64 - b_len as f64) / b_len as f64 * 100.0).abs();
        if delta_pct > 50.0 {
            diffs.push(DiffEntry {
                dimension: DiffDimension::Output,
                severity: DiffSeverity::Soft,
                category: DiffCategory::Functional,
                raw_diff: format!("answer_text_length: {} -> {}", b_len, c_len),
                normalized_signal: format!("answer_text_length: {} -> {} ({:.0}%)", b_len, c_len, delta_pct),
                baseline_value: serde_json::json!(b_len),
                current_value: serde_json::json!(c_len),
            });
        }
    }

    // HTML presence change
    match (&baseline.answer_html, &current.answer_html) {
        (Some(_), None) | (None, Some(_)) => {
            diffs.push(DiffEntry {
                dimension: DiffDimension::Output,
                severity: DiffSeverity::Info,
                category: DiffCategory::Functional,
                raw_diff: "answer_html presence changed".to_string(),
                normalized_signal: "answer_html: presence changed".to_string(),
                baseline_value: serde_json::json!(baseline.answer_html.is_some()),
                current_value: serde_json::json!(current.answer_html.is_some()),
            });
        }
        _ => {}
    }

    diffs
}
```

- [ ] **Step 3: Add cost/perf diff**

Append to `diff.rs`:

```rust
// ---------------------------------------------------------------------------
// Cost/Perf Diff (Non-Functional)
// ---------------------------------------------------------------------------

pub fn compare_cost_perf(baseline: &TestResult, current: &TestResult) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // duration_ms: +30% relative or >20s absolute
    let b_dur = baseline.duration_ms as f64;
    let c_dur = current.duration_ms as f64;
    if b_dur > 0.0 {
        let rel_change = (c_dur - b_dur) / b_dur;
        let hard_threshold = 0.30;
        let abs_threshold = 20_000.0;
        if rel_change > hard_threshold || c_dur > abs_threshold {
            diffs.push(DiffEntry {
                dimension: DiffDimension::CostPerf,
                severity: DiffSeverity::Hard,
                category: DiffCategory::NonFunctional,
                raw_diff: format!("duration_ms: {} -> {}", baseline.duration_ms, current.duration_ms),
                normalized_signal: format!("duration_ms: {} -> {} ({:.0}%)", baseline.duration_ms, current.duration_ms, rel_change * 100.0),
                baseline_value: serde_json::json!(baseline.duration_ms),
                current_value: serde_json::json!(current.duration_ms),
            });
        }
    }

    // token usage
    if let (Some(b_tok), Some(c_tok)) = (&baseline.token_usage, &current.token_usage) {
        // total_input_tokens (prompt_tokens): +30% or >20k
        let b_in = b_tok.prompt_tokens as f64;
        let c_in = c_tok.prompt_tokens as f64;
        if b_in > 0.0 {
            let rel = (c_in - b_in) / b_in;
            if rel > 0.30 || c_in > 20_000.0 {
                diffs.push(DiffEntry {
                    dimension: DiffDimension::CostPerf,
                    severity: DiffSeverity::Hard,
                    category: DiffCategory::NonFunctional,
                    raw_diff: format!("prompt_tokens: {} -> {}", b_tok.prompt_tokens, c_tok.prompt_tokens),
                    normalized_signal: format!("prompt_tokens: {} -> {} ({:.0}%)", b_tok.prompt_tokens, c_tok.prompt_tokens, rel * 100.0),
                    baseline_value: serde_json::json!(b_tok.prompt_tokens),
                    current_value: serde_json::json!(c_tok.prompt_tokens),
                });
            }
        }

        // total_output_tokens (completion_tokens): +30% or >10k
        let b_out = b_tok.completion_tokens as f64;
        let c_out = c_tok.completion_tokens as f64;
        if b_out > 0.0 {
            let rel = (c_out - b_out) / b_out;
            if rel > 0.30 || c_out > 10_000.0 {
                diffs.push(DiffEntry {
                    dimension: DiffDimension::CostPerf,
                    severity: DiffSeverity::Hard,
                    category: DiffCategory::NonFunctional,
                    raw_diff: format!("completion_tokens: {} -> {}", b_tok.completion_tokens, c_tok.completion_tokens),
                    normalized_signal: format!("completion_tokens: {} -> {} ({:.0}%)", b_tok.completion_tokens, c_tok.completion_tokens, rel * 100.0),
                    baseline_value: serde_json::json!(b_tok.completion_tokens),
                    current_value: serde_json::json!(c_tok.completion_tokens),
                });
            }
        }
    }

    // llm_call_count: +50% or >10
    let b_llm = baseline.llm_calls.len() as f64;
    let c_llm = current.llm_calls.len() as f64;
    if b_llm > 0.0 {
        let rel = (c_llm - b_llm) / b_llm;
        if rel > 0.50 || c_llm > 10.0 {
            diffs.push(DiffEntry {
                dimension: DiffDimension::CostPerf,
                severity: DiffSeverity::Hard,
                category: DiffCategory::NonFunctional,
                raw_diff: format!("llm_call_count: {} -> {}", baseline.llm_calls.len(), current.llm_calls.len()),
                normalized_signal: format!("llm_call_count: {} -> {} ({:.0}%)", baseline.llm_calls.len(), current.llm_calls.len(), rel * 100.0),
                baseline_value: serde_json::json!(baseline.llm_calls.len()),
                current_value: serde_json::json!(current.llm_calls.len()),
            });
        }
    }

    // tool_call_count: +50%
    let b_tools = baseline.tool_calls.len() as f64;
    let c_tools = current.tool_calls.len() as f64;
    if b_tools > 0.0 {
        let rel = (c_tools - b_tools) / b_tools;
        if rel > 0.50 {
            diffs.push(DiffEntry {
                dimension: DiffDimension::CostPerf,
                severity: DiffSeverity::Hard,
                category: DiffCategory::NonFunctional,
                raw_diff: format!("tool_call_count: {} -> {}", baseline.tool_calls.len(), current.tool_calls.len()),
                normalized_signal: format!("tool_call_count: {} -> {} ({:.0}%)", baseline.tool_calls.len(), current.tool_calls.len(), rel * 100.0),
                baseline_value: serde_json::json!(baseline.tool_calls.len()),
                current_value: serde_json::json!(current.tool_calls.len()),
            });
        }
    }

    diffs
}
```

- [ ] **Step 4: Add tests for behavior/output/cost dimensions**

Append to the `#[cfg(test)]` block at the bottom of `diff.rs`:

```rust
    #[test]
    fn test_behavior_status_change_passed_to_failed() {
        let mut b = dummy_result();
        b.status = TestStatus::Passed;
        let mut c = dummy_result();
        c.status = TestStatus::Failed;

        let diffs = compare_behavior(&b, &c);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].severity, DiffSeverity::Hard));
    }

    #[test]
    fn test_output_html_presence_change() {
        let mut b = dummy_result();
        b.answer_html = Some("<html></html>".to_string());
        let c = dummy_result();

        let diffs = compare_output(&b, &c);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].dimension, DiffDimension::Output));
    }

    #[test]
    fn test_cost_perf_duration_regression() {
        let mut b = dummy_result();
        b.duration_ms = 1000;
        let mut c = dummy_result();
        c.duration_ms = 1500; // +50%

        let diffs = compare_cost_perf(&b, &c);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].severity, DiffSeverity::Hard));
        assert!(matches!(diffs[0].dimension, DiffDimension::CostPerf));
    }
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo test -p e2e-analyzer diff`
Expected: all diff tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add behavior, output, and cost/perf diff dimensions"
```

---

### Task 7: Report generation (Markdown + JSON)

**Files:**
- Create: `crates/e2e-analyzer/src/report.rs`

- [ ] **Step 1: Write report.rs**

```rust
use crate::models::{AttributionReport, DiffEntry, DiffSeverity, GateStatus, JsonSummary, SeveritySummary, TestResult};

/// Generate a Markdown regression report.
pub fn generate_markdown_report(
    baseline_run_id: &str,
    current_run_id: &str,
    baseline_results: &[TestResult],
    current_results: &[TestResult],
    diffs: &[(String, Vec<DiffEntry>)],
    attributions: &[AttributionReport],
) -> String {
    let mut md = String::new();
    md.push_str("# E2E Regression Report\n\n");

    // Summary
    let summary = summarize_diffs(diffs);
    md.push_str(&format!("- **Baseline:** `{}`\n", baseline_run_id));
    md.push_str(&format!("- **Current:** `{}`\n", current_run_id));
    md.push_str(&format!(
        "- {} passed, {} soft drift, {} hard regression\n\n",
        current_results.iter().filter(|r| matches!(r.status, crate::models::TestStatus::Passed)).count(),
        summary.soft, summary.hard
    ));

    // Hard regressions
    let hard_diffs: Vec<_> = diffs
        .iter()
        .flat_map(|(name, entries)| {
            entries
                .iter()
                .filter(|e| matches!(e.severity, DiffSeverity::Hard))
                .map(move |e| (name.clone(), e))
        })
        .collect();

    if !hard_diffs.is_empty() {
        md.push_str("## Hard Regressions\n\n");
        for (test_name, entry) in &hard_diffs {
            md.push_str(&format!("### {}\n", test_name));
            md.push_str(&format!("- **Category:** {:?}\n", entry.category));
            md.push_str(&format!("- **Dimension:** {:?}\n", entry.dimension));
            md.push_str(&format!("- **Signal:** `{}`\n", entry.normalized_signal));
            md.push_str(&format!("- **Detail:** {}\n\n", entry.raw_diff));
        }
    }

    // Soft diffs
    let soft_diffs: Vec<_> = diffs
        .iter()
        .flat_map(|(name, entries)| {
            entries
                .iter()
                .filter(|e| matches!(e.severity, DiffSeverity::Soft))
                .map(move |e| (name.clone(), e))
        })
        .collect();

    if !soft_diffs.is_empty() {
        md.push_str("## Soft Drift\n\n");
        for (test_name, entry) in &soft_diffs {
            md.push_str(&format!("- **{}**: `{}` ({:?})\n", test_name, entry.normalized_signal, entry.dimension));
        }
        md.push('\n');
    }

    // Info diffs
    let info_diffs: Vec<_> = diffs
        .iter()
        .flat_map(|(name, entries)| {
            entries
                .iter()
                .filter(|e| matches!(e.severity, DiffSeverity::Info))
                .map(move |e| (name.clone(), e))
        })
        .collect();

    if !info_diffs.is_empty() {
        md.push_str("## Info\n\n");
        for (test_name, entry) in &info_diffs {
            md.push_str(&format!("- **{}**: `{}` ({:?})\n", test_name, entry.normalized_signal, entry.dimension));
        }
        md.push('\n');
    }

    // Attribution section
    if !attributions.is_empty() {
        md.push_str("## Failure Attribution\n\n");
        for attr in attributions {
            md.push_str(&format!("### {}\n", attr.test_name));
            md.push_str(&format!("- **Category:** {:?}\n", attr.category));
            md.push_str(&format!("- **Confidence:** {:?}\n", attr.confidence));
            md.push_str(&format!("- **Suspected Layers:** {:?}\n", attr.suspected_layers));
            md.push_str(&format!("- **Suggested Action:** {}\n", attr.suggested_action));
            if let Some(ref note) = attr.diagnostic_notes {
                md.push_str(&format!("- **Notes:** {}\n", note));
            }
            md.push('\n');
        }
    }

    md
}

/// Generate JSON summary for CI gate parsing.
pub fn generate_json_summary(
    baseline_run_id: &str,
    current_run_id: &str,
    diffs: &[(String, Vec<DiffEntry>)],
) -> JsonSummary {
    let summary = summarize_diffs(diffs);
    JsonSummary {
        baseline_run_id: baseline_run_id.to_string(),
        current_run_id: current_run_id.to_string(),
        summary: summary.clone(),
        gate_status: summary.to_gate_status(),
    }
}

fn summarize_diffs(diffs: &[(String, Vec<DiffEntry>)]) -> SeveritySummary {
    let mut summary = SeveritySummary::default();
    for (_, entries) in diffs {
        for entry in entries {
            match entry.severity {
                DiffSeverity::Hard => summary.hard += 1,
                DiffSeverity::Soft => summary.soft += 1,
                DiffSeverity::Info => summary.info += 1,
            }
        }
    }
    summary
}

/// Determine exit code for CI gate.
pub fn exit_code(summary: &SeveritySummary) -> i32 {
    match summary.to_gate_status() {
        GateStatus::Blocked => 1,
        GateStatus::ReviewRequired => 0, // soft = warning but not block
        GateStatus::Pass => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DiffCategory, DiffDimension};

    fn make_diff(severity: DiffSeverity) -> DiffEntry {
        DiffEntry {
            dimension: DiffDimension::Prompt,
            severity,
            category: DiffCategory::Functional,
            raw_diff: "test".to_string(),
            normalized_signal: "test".to_string(),
            baseline_value: serde_json::Value::Null,
            current_value: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_summarize_diffs_counts_correctly() {
        let diffs = vec![
            ("t1".to_string(), vec![make_diff(DiffSeverity::Hard), make_diff(DiffSeverity::Soft)]),
            ("t2".to_string(), vec![make_diff(DiffSeverity::Info)]),
        ];
        let summary = summarize_diffs(&diffs);
        assert_eq!(summary.hard, 1);
        assert_eq!(summary.soft, 1);
        assert_eq!(summary.info, 1);
    }

    #[test]
    fn test_exit_code_blocked_on_hard() {
        let summary = SeveritySummary { hard: 1, soft: 0, info: 0 };
        assert_eq!(exit_code(&summary), 1);
    }

    #[test]
    fn test_exit_code_pass_on_soft_only() {
        let summary = SeveritySummary { hard: 0, soft: 2, info: 3 };
        assert_eq!(exit_code(&summary), 0);
    }
}
```

- [ ] **Step 2: Add report module to main.rs**

```rust
mod baseline;
mod cli;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p e2e-analyzer report`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add markdown and json report generation"
```

---

### Task 8: Failure attribution engine (Phase 2)

**Files:**
- Create: `crates/e2e-analyzer/src/attribution.rs`

- [ ] **Step 1: Write attribution.rs**

```rust
use crate::models::*;

/// Map Phase 1 diffs to attribution reports.
pub fn attribute_failures(
    test_name: &str,
    fingerprint_match: bool,
    diffs: &[DiffEntry],
    baseline: &TestResult,
    current: &TestResult,
) -> Option<AttributionReport> {
    // If no diffs and current passed, no attribution needed
    if diffs.is_empty() && matches!(current.status, TestStatus::Passed) {
        return None;
    }

    // Priority chain (first match wins)
    let (category, suspected_layers, confidence, suggested_action) =
        if let Some(attr) = classify_state_machine_failure(diffs, current) {
            attr
        } else if let Some(attr) = classify_prompt_assembly_failure(diffs) {
            attr
        } else if let Some(attr) = classify_tool_execution_failure(diffs, current) {
            attr
        } else if let Some(attr) = classify_model_behavior_failure(diffs, current) {
            attr
        } else if let Some(attr) = classify_performance_regression(diffs) {
            attr
        } else {
            // Unknown — generic attribution
            (
                FailureCategory::ModelBehaviorFailure,
                vec![SuspectedLayer::LlmOutput],
                ConfidenceLevel::Low,
                "Review manually — no clear attribution pattern matched.".to_string(),
            )
        };

    let severity = diffs
        .iter()
        .map(|d| d.severity)
        .max_by_key(|s| match s {
            DiffSeverity::Hard => 3,
            DiffSeverity::Soft => 2,
            DiffSeverity::Info => 1,
        })
        .unwrap_or(DiffSeverity::Info);

    let first_anomaly = locate_first_anomaly(diffs, current);

    let diagnostic_notes = if !fingerprint_match {
        Some("Fingerprint mismatch — attribution is informational only.".to_string())
    } else {
        None
    };

    Some(AttributionReport {
        test_name: test_name.to_string(),
        fingerprint_match,
        category,
        severity,
        confidence: if !fingerprint_match { ConfidenceLevel::Low } else { confidence },
        suspected_layers,
        first_anomaly,
        related_diffs: diffs.to_vec(),
        suggested_action,
        diagnostic_notes,
    })
}

fn classify_state_machine_failure(
    diffs: &[DiffEntry],
    current: &TestResult,
) -> Option<(FailureCategory, Vec<SuspectedLayer>, ConfidenceLevel, String)> {
    // Illegal state transition: status changed from Passed to Failed with tool error
    let has_status_regression = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::Behavior)
            && d.raw_diff.contains("status: Passed -> Failed")
    });

    let has_tool_error = current.tool_calls.iter().any(|t| {
        t.status.to_lowercase() == "error"
    });

    if has_status_regression && has_tool_error {
        return Some((
            FailureCategory::StateMachineFailure,
            vec![SuspectedLayer::Fsm],
            ConfidenceLevel::High,
            "Check state transition logic and tool dispatch ordering.".to_string(),
        ));
    }

    None
}

fn classify_prompt_assembly_failure(
    diffs: &[DiffEntry],
) -> Option<(FailureCategory, Vec<SuspectedLayer>, ConfidenceLevel, String)> {
    // Missing skill/tool in catalog
    let has_missing_tool = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::Prompt)
            && d.raw_diff.contains("missing_tools")
    });

    if has_missing_tool {
        return Some((
            FailureCategory::PromptAssemblyFailure,
            vec![SuspectedLayer::PromptAssembly],
            ConfidenceLevel::High,
            "Verify tool catalog registration and skill injection pipeline.".to_string(),
        ));
    }

    // System prompt hash changed + assertion failure
    let has_prompt_change = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::Prompt)
            && d.raw_diff.contains("system_prompt_hash")
    });

    if has_prompt_change {
        return Some((
            FailureCategory::PromptAssemblyFailure,
            vec![SuspectedLayer::PromptAssembly],
            ConfidenceLevel::Medium,
            "Review prompt assembly changes — skill body or tool catalog may have drifted.".to_string(),
        ));
    }

    None
}

fn classify_tool_execution_failure(
    diffs: &[DiffEntry],
    current: &TestResult,
) -> Option<(FailureCategory, Vec<SuspectedLayer>, ConfidenceLevel, String)> {
    let has_tool_error = current.tool_calls.iter().any(|t| {
        t.status.to_lowercase() == "error"
    });

    if has_tool_error {
        return Some((
            FailureCategory::ToolExecutionFailure,
            vec![SuspectedLayer::ToolDispatch],
            ConfidenceLevel::High,
            "Investigate tool error response and retry configuration.".to_string(),
        ));
    }

    // Tool count increase suggests replan/retry loop
    let has_tool_count_increase = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::Behavior)
            && d.raw_diff.contains("tool_call_count")
    });

    if has_tool_count_increase {
        return Some((
            FailureCategory::ToolExecutionFailure,
            vec![SuspectedLayer::ToolDispatch, SuspectedLayer::Fsm],
            ConfidenceLevel::Medium,
            "Tool call count increased — check for retry loops or dispatch failures.".to_string(),
        ));
    }

    None
}

fn classify_model_behavior_failure(
    diffs: &[DiffEntry],
    current: &TestResult,
) -> Option<(FailureCategory, Vec<SuspectedLayer>, ConfidenceLevel, String)> {
    // Output format drift
    let has_output_drift = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::Output)
    });

    if has_output_drift && !matches!(current.status, TestStatus::Passed) {
        return Some((
            FailureCategory::ModelBehaviorFailure,
            vec![SuspectedLayer::LlmOutput],
            ConfidenceLevel::Medium,
            "Output structure drift detected — verify format constraints and model behavior.".to_string(),
        ));
    }

    None
}

fn classify_performance_regression(
    diffs: &[DiffEntry],
) -> Option<(FailureCategory, Vec<SuspectedLayer>, ConfidenceLevel, String)> {
    let has_perf_regression = diffs.iter().any(|d| {
        matches!(d.dimension, DiffDimension::CostPerf)
            && matches!(d.severity, DiffSeverity::Hard)
    });

    if has_perf_regression {
        return Some((
            FailureCategory::PerformanceRegression,
            vec![SuspectedLayer::PerfBudget],
            ConfidenceLevel::High,
            "Investigate token/call count increase and optimize prompt or tool selection.".to_string(),
        ));
    }

    None
}

fn locate_first_anomaly(diffs: &[DiffEntry], current: &TestResult) -> Option<FirstAnomaly> {
    // Simplified first anomaly: find the earliest tool error or status change
    if let Some(tool_err) = current.tool_calls.iter().find(|t| t.status.to_lowercase() == "error") {
        return Some(FirstAnomaly {
            stage: "execute".to_string(),
            iteration: 0,
            expected_next: vec!["ok".to_string()],
            actual_next: tool_err.status.clone(),
            reasoning: format!("Tool '{}' returned error status", tool_err.tool_id),
        });
    }

    if diffs.iter().any(|d| d.raw_diff.contains("status: Passed -> Failed")) {
        return Some(FirstAnomaly {
            stage: "evaluate".to_string(),
            iteration: 0,
            expected_next: vec!["Passed".to_string()],
            actual_next: "Failed".to_string(),
            reasoning: "Test status regressed from Passed to Failed".to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_result() -> TestResult {
        TestResult {
            run_id: "r1".to_string(),
            test_name: "t1".to_string(),
            query: "q".to_string(),
            strategy: "Chat".to_string(),
            format_skill: None,
            status: TestStatus::Failed,
            answer_text: "".to_string(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: vec![],
            tool_calls: vec![],
            retrieval_hits: None,
            token_usage: None,
            duration_ms: 1000,
            timestamp: "2026-05-28T00:00:00Z".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_attribute_missing_tool_is_prompt_assembly() {
        let diffs = vec![DiffEntry {
            dimension: DiffDimension::Prompt,
            severity: DiffSeverity::Hard,
            category: DiffCategory::Functional,
            raw_diff: "missing_tools_in_current: [\"calculator\"]".to_string(),
            normalized_signal: "tool_catalog_count: 3 -> 2".to_string(),
            baseline_value: serde_json::Value::Null,
            current_value: serde_json::Value::Null,
        }];
        let current = dummy_result();
        let baseline = dummy_result();
        baseline.status = TestStatus::Passed;

        let attr = attribute_failures("test", true, &diffs, &baseline, &current).unwrap();
        assert!(matches!(attr.category, FailureCategory::PromptAssemblyFailure));
        assert!(matches!(attr.confidence, ConfidenceLevel::High));
    }

    #[test]
    fn test_attribute_tool_error_is_tool_execution() {
        let mut current = dummy_result();
        current.tool_calls.push(ToolCallRecord {
            tool_id: "web_search".to_string(),
            input: serde_json::json!({}),
            output: serde_json::json!({"error": "timeout"}),
            status: "error".to_string(),
        });
        let baseline = dummy_result();
        baseline.status = TestStatus::Passed;

        let attr = attribute_failures("test", true, &[], &baseline, &current).unwrap();
        assert!(matches!(attr.category, FailureCategory::ToolExecutionFailure));
        assert!(matches!(attr.confidence, ConfidenceLevel::High));
    }

    #[test]
    fn test_no_attribution_when_passed_and_no_diffs() {
        let mut current = dummy_result();
        current.status = TestStatus::Passed;
        let baseline = dummy_result();
        baseline.status = TestStatus::Passed;

        let attr = attribute_failures("test", true, &[], &baseline, &current);
        assert!(attr.is_none());
    }
}
```

- [ ] **Step 2: Add attribution module to main.rs**

```rust
mod attribution;
mod baseline;
mod cli;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p e2e-analyzer attribution`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add failure attribution engine with priority chain"
```

---

### Task 9: Wire up CLI commands — diff, diagnose, baseline, report

**Files:**
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Implement CLI dispatch**

Replace the entire `main.rs` with:

```rust
mod attribution;
mod baseline;
mod cli;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;

use clap::Parser;
use std::path::PathBuf;

fn main() {
    let cli = cli::Cli::parse();
    let output_dir = resolve_output_dir(&cli.output_dir);

    match cli.command {
        cli::Commands::Diff { baseline_run_id, current_run_id } => {
            cmd_diff(&output_dir, baseline_run_id.as_deref(), &current_run_id);
        }
        cli::Commands::Diagnose { run_id, baseline_run_id } => {
            cmd_diagnose(&output_dir, &run_id, baseline_run_id.as_deref());
        }
        cli::Commands::Coverage { runs } => {
            println!("Coverage analysis not yet implemented (P1). Requested last {} runs.", runs);
        }
        cli::Commands::Trends { test_name, runs } => {
            println!("Trends analysis not yet implemented (P2). Test: {}, runs: {}.", test_name, runs);
        }
        cli::Commands::Report { current_run_id, baseline_run_id } => {
            cmd_report(&output_dir, baseline_run_id.as_deref(), &current_run_id);
        }
        cli::Commands::Baseline { action } => match action {
            cli::BaselineAction::Promote { run_id } => {
                baseline::write_persistent_baseline(&output_dir, &run_id).unwrap();
                println!("Baseline promoted to: {}", run_id);
            }
            cli::BaselineAction::Show => {
                match baseline::read_persistent_baseline(&output_dir) {
                    Some(id) => println!("Current baseline: {}", id),
                    None => println!("No persistent baseline set."),
                }
            }
        },
    }
}

fn resolve_output_dir(path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir().unwrap().join(path)
    }
}

fn cmd_diff(output_dir: &PathBuf, baseline_run_id: Option<&str>, current_run_id: &str) {
    let current_dir = loader::find_run_dir(output_dir, current_run_id)
        .unwrap_or_else(|| panic!("Current run not found: {}", current_run_id));
    let current_results = loader::load_run_results(&current_dir);

    let baseline_dir = baseline::resolve_baseline(output_dir, baseline_run_id, &current_dir)
        .unwrap_or_else(|| panic!("No baseline found for comparison"));
    let baseline_results = loader::load_run_results(&baseline_dir);

    let baseline_meta = loader::load_run_metadata(&baseline_dir);
    let baseline_id = baseline_meta.map(|m| m.run_id).unwrap_or_else(|| {
        baseline_dir.file_name().unwrap().to_string_lossy().to_string()
    });

    let mut all_diffs: Vec<(String, Vec<models::DiffEntry>)> = Vec::new();

    for current in &current_results {
        let baseline = baseline_results.iter().find(|b| b.test_name == current.test_name);
        if let Some(baseline) = baseline {
            let diffs = diff::compare_runs(baseline, current);
            if !diffs.is_empty() {
                all_diffs.push((current.test_name.clone(), diffs));
            }
        }
    }

    let summary = report::generate_json_summary(&baseline_id, current_run_id, &all_diffs);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());

    // Also print markdown to stdout
    let md = report::generate_markdown_report(
        &baseline_id, current_run_id, &baseline_results, &current_results, &all_diffs, &[],
    );
    println!("\n{}", md);

    std::process::exit(report::exit_code(&summary.summary));
}

fn cmd_diagnose(output_dir: &PathBuf, run_id: &str, baseline_run_id: Option<&str>) {
    let current_dir = loader::find_run_dir(output_dir, run_id)
        .unwrap_or_else(|| panic!("Run not found: {}", run_id));
    let current_results = loader::load_run_results(&current_dir);

    let baseline_dir = baseline::resolve_baseline(output_dir, baseline_run_id, &current_dir);
    let baseline_results = baseline_dir.as_ref()
        .map(|d| loader::load_run_results(d))
        .unwrap_or_default();

    let baseline_meta = baseline_dir.as_ref().and_then(|d| loader::load_run_metadata(d));
    let baseline_id = baseline_meta.map(|m| m.run_id).unwrap_or_else(|| "unknown".to_string());

    let mut all_diffs: Vec<(String, Vec<models::DiffEntry>)> = Vec::new();
    let mut attributions: Vec<models::AttributionReport> = Vec::new();

    for current in &current_results {
        let baseline = baseline_results.iter().find(|b| b.test_name == current.test_name);
        let diffs = if let Some(baseline) = baseline {
            diff::compare_runs(baseline, current)
        } else {
            Vec::new()
        };

        if !diffs.is_empty() || !matches!(current.status, models::TestStatus::Passed) {
            all_diffs.push((current.test_name.clone(), diffs.clone()));
        }

        let fp_baseline = fingerprint::fingerprint_for_test(&current.test_name);
        let fp_current = fingerprint::fingerprint_for_test(&current.test_name);
        let fingerprint_match = match (&fp_baseline, &fp_current) {
            (Some(a), Some(b)) => fingerprint::fingerprint_match(&a.source_hash, &b.source_hash),
            _ => false,
        };

        let baseline_test = baseline.cloned().unwrap_or_else(|| current.clone());
        if let Some(attr) = attribution::attribute_failures(
            &current.test_name,
            fingerprint_match,
            &diffs,
            &baseline_test,
            current,
        ) {
            attributions.push(attr);
        }
    }

    let md = report::generate_markdown_report(
        &baseline_id, run_id, &baseline_results, &current_results, &all_diffs, &attributions,
    );
    println!("{}", md);

    let summary = report::summarize_diffs(&all_diffs);
    std::process::exit(report::exit_code(&summary));
}

fn cmd_report(output_dir: &PathBuf, baseline_run_id: Option<&str>, current_run_id: &str) {
    // Report runs diff + diagnose combined
    cmd_diagnose(output_dir, current_run_id, baseline_run_id);
}
```

- [ ] **Step 2: Add serde_json pretty-print helper to report.rs**

Add this function to `report.rs`:

```rust
pub fn summarize_diffs(diffs: &[(String, Vec<DiffEntry>)]) -> SeveritySummary {
    let mut summary = SeveritySummary::default();
    for (_, entries) in diffs {
        for entry in entries {
            match entry.severity {
                DiffSeverity::Hard => summary.hard += 1,
                DiffSeverity::Soft => summary.soft += 1,
                DiffSeverity::Info => summary.info += 1,
            }
        }
    }
    summary
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p e2e-analyzer`
Expected: clean compile.

- [ ] **Step 4: Test CLI with existing data**

Run:
```bash
cd /home/chuan/context-osv6/avrag-rs
cargo run -p e2e-analyzer -- baseline show
cargo run -p e2e-analyzer -- diff --current-run-id e2e_20260528-042441_555871ea 2>/dev/null | head -20
```
Expected: baseline show prints "No persistent baseline set." Diff prints JSON summary + markdown report.

- [ ] **Step 5: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): wire up diff, diagnose, baseline, report CLI commands"
```

---

## Milestone 2: Phase 3 — Coverage Governance Matrix (P1)

### Task 10: Coverage matrix scanner

**Files:**
- Modify: `crates/e2e-analyzer/src/coverage.rs`
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Write coverage.rs**

```rust
use crate::models::{CoverageGap, GapPriority, TestResult, TestStatus};
use std::collections::{HashMap, HashSet};

/// Core dimensions for coverage analysis.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoverageDimension {
    Strategy,
    StateBoundary,
    OutputFormat,
    RiskCategory,
}

/// Scan multiple runs and build coverage matrix.
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

            // Output format dimension
            if let Some(ref fmt) = result.format_skill {
                matrix.hit(
                    CoverageDimension::OutputFormat,
                    fmt.clone(),
                    result.test_name.clone(),
                    result.status,
                );
            }

            // Risk category: infer from test name patterns
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

#[derive(Debug, Default)]
pub struct CoverageMatrix {
    cells: HashMap<(CoverageDimension, String), CoverageCell>,
}

#[derive(Debug, Default)]
pub struct CoverageCell {
    pub test_names: HashSet<String>,
    pub passed_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}

impl CoverageMatrix {
    fn hit(&mut self, dim: CoverageDimension, value: String, test_name: String, status: TestStatus) {
        let cell = self.cells.entry((dim, value)).or_default();
        cell.test_names.insert(test_name);
        match status {
            TestStatus::Passed => cell.passed_count += 1,
            TestStatus::Failed => cell.failed_count += 1,
            TestStatus::Skipped => cell.skipped_count += 1,
        }
    }

    pub fn gaps(&self) -> Vec<CoverageGap> {
        let mut gaps = Vec::new();

        // Check for never-tested or only-failing dimensions
        for ((dim, value), cell) in &self.cells {
            let total = cell.passed_count + cell.failed_count + cell.skipped_count;
            let flaky_rate = if total > 0 {
                cell.failed_count as f32 / total as f32
            } else {
                0.0
            };

            if cell.passed_count == 0 && cell.failed_count > 0 {
                gaps.push(CoverageGap {
                    priority: GapPriority::High,
                    risk_score: 0.9,
                    dimensions: [(format!("{:?}", dim), value.clone())].into_iter().collect(),
                    related_tests: cell.test_names.iter().cloned().collect(),
                    evidence: format!("All {} runs failed for {:?}={}", total, dim, value),
                    recommended_test_pattern: format!("Add passing test for {:?}={}", dim, value),
                });
            } else if flaky_rate > 0.2 {
                gaps.push(CoverageGap {
                    priority: GapPriority::Medium,
                    risk_score: flaky_rate,
                    dimensions: [(format!("{:?}", dim), value.clone())].into_iter().collect(),
                    related_tests: cell.test_names.iter().cloned().collect(),
                    evidence: format!("Flaky rate {:.0}% for {:?}={}", flaky_rate * 100.0, dim, value),
                    recommended_test_pattern: format!("Stabilize test for {:?}={}", dim, value),
                });
            }
        }

        gaps.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap());
        gaps
    }
}

fn infer_risk_category(test_name: &str) -> String {
    let name_lower = test_name.to_lowercase();
    if name_lower.contains("inject") || name_lower.contains("guard") || name_lower.contains("jailbreak") {
        "injection".to_string()
    } else if name_lower.contains("empty") || name_lower.contains("budget") {
        "empty_input".to_string()
    } else if name_lower.contains("cancel") {
        "cancellation".to_string()
    } else if name_lower.contains("format") || name_lower.contains("ppt") || name_lower.contains("html") {
        "format_constraint".to_string()
    } else {
        "general".to_string()
    }
}

/// Generate Markdown coverage report.
pub fn generate_coverage_report(gaps: &[CoverageGap]) -> String {
    let mut md = String::new();
    md.push_str("# E2E Coverage Governance Report\n\n");

    if gaps.is_empty() {
        md.push_str("No coverage gaps detected.\n");
        return md;
    }

    md.push_str("## Coverage Gaps\n\n");
    md.push_str("| Priority | Risk Score | Dimension | Evidence | Recommended Pattern |\n");
    md.push_str("|----------|-----------|-----------|----------|---------------------|\n");

    for gap in gaps {
        let dim_str = gap
            .dimensions
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        md.push_str(&format!(
            "| {:?} | {:.2} | {} | {} | {} |\n",
            gap.priority, gap.risk_score, dim_str, gap.evidence, gap.recommended_test_pattern
        ));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(name: &str, strategy: &str, status: TestStatus) -> TestResult {
        TestResult {
            run_id: "r".to_string(),
            test_name: name.to_string(),
            query: "q".to_string(),
            strategy: strategy.to_string(),
            format_skill: None,
            status,
            answer_text: "".to_string(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: vec![],
            tool_calls: vec![],
            retrieval_hits: None,
            token_usage: None,
            duration_ms: 1000,
            timestamp: "".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_coverage_detects_all_failing() {
        let run1 = vec![
            make_result("test_a", "Chat", TestStatus::Failed),
        ];
        let run2 = vec![
            make_result("test_a", "Chat", TestStatus::Failed),
        ];

        let matrix = build_coverage_matrix(&[run1, run2]);
        let gaps = matrix.gaps();
        assert!(!gaps.is_empty());
        assert!(matches!(gaps[0].priority, GapPriority::High));
    }

    #[test]
    fn test_infer_risk_category() {
        assert_eq!(infer_risk_category("chat_content_guard"), "injection");
        assert_eq!(infer_risk_category("search_budget_exhaustion"), "empty_input");
        assert_eq!(infer_risk_category("chat_ppt_format"), "format_constraint");
    }
}
```

- [ ] **Step 2: Wire up coverage command in main.rs**

In the `cli::Commands::Coverage` arm, replace the placeholder with:

```rust
        cli::Commands::Coverage { runs } => {
            let all_runs = loader::discover_runs(&output_dir);
            let recent_runs: Vec<_> = all_runs.iter().rev().take(runs).cloned().collect();
            let run_results: Vec<_> = recent_runs.iter().map(|d| loader::load_run_results(d)).collect();
            let matrix = coverage::build_coverage_matrix(&run_results);
            let gaps = matrix.gaps();
            let report = coverage::generate_coverage_report(&gaps);
            println!("{}", report);
        }
```

- [ ] **Step 3: Add coverage module to main.rs**

```rust
mod attribution;
mod baseline;
mod cli;
mod coverage;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p e2e-analyzer coverage`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add coverage governance matrix (P1)"
```

---

## Milestone 3: Phase 4 — Stability and Trending (P2)

### Task 11: Flaky detection and performance trends

**Files:**
- Create: `crates/e2e-analyzer/src/stability.rs`
- Modify: `crates/e2e-analyzer/src/main.rs`

- [ ] **Step 1: Write stability.rs**

```rust
use crate::models::{CategorySnapshot, DiffDimension, DiffSeverity, DriftWarning, PerfRegression, PerfTrend, StabilityRecord, TestResult, TestStatus};
use std::collections::HashMap;

/// Analyze test stability across multiple runs.
pub fn analyze_stability(
    test_name: &str,
    runs: &[(String, Vec<TestResult>)], // (run_id, results)
) -> Option<StabilityRecord> {
    let test_results: Vec<_> = runs
        .iter()
        .filter_map(|(run_id, results)| {
            results
                .iter()
                .find(|r| r.test_name == test_name)
                .map(|r| (run_id.clone(), r))
        })
        .collect();

    if test_results.len() < 2 {
        return None;
    }

    let total = test_results.len();
    let passed = test_results.iter().filter(|(_, r)| matches!(r.status, TestStatus::Passed)).count();
    let flaky_rate = if total > 0 {
        (total - passed) as f32 / total as f32
    } else {
        0.0
    };

    let mut consecutive_failures = 0u32;
    for (_, result) in test_results.iter().rev() {
        if matches!(result.status, TestStatus::Failed) {
            consecutive_failures += 1;
        } else {
            break;
        }
    }

    let category_history: Vec<_> = test_results
        .iter()
        .map(|(run_id, result)| CategorySnapshot {
            run_id: run_id.clone(),
            category: if matches!(result.status, TestStatus::Failed) {
                Some(crate::models::FailureCategory::ModelBehaviorFailure)
            } else {
                None
            },
        })
        .collect();

    let perf_trend = analyze_perf_trend(&test_results);

    Some(StabilityRecord {
        test_name: test_name.to_string(),
        fingerprint_hash: String::new(), // populated by caller
        flaky_rate,
        runs_analyzed: total as u32,
        consecutive_failures,
        category_history,
        perf_trend,
    })
}

fn analyze_perf_trend(test_results: &[(String, &TestResult)]) -> PerfTrend {
    let mut trend = PerfTrend::default();

    // Duration hard regressions
    for (run_id, result) in test_results {
        if result.duration_ms > 20_000 {
            trend.hard_regressions.push(PerfRegression {
                run_id: run_id.clone(),
                metric: "duration_ms".to_string(),
                value: result.duration_ms as f64,
                threshold: 20_000.0,
            });
        }
    }

    // Token drift warnings: N-run slope positive
    let token_values: Vec<(String, f64)> = test_results
        .iter()
        .filter_map(|(run_id, r)| {
            r.token_usage.as_ref().map(|u| {
                let total = u.prompt_tokens + u.completion_tokens;
                (run_id.clone(), total as f64)
            })
        })
        .collect();

    if token_values.len() >= 5 {
        let slope = compute_linear_slope(&token_values);
        if slope > 0.0 {
            trend.drift_warnings.push(DriftWarning {
                metric: "total_tokens".to_string(),
                slope,
                runs_window: token_values.len(),
                values: token_values.iter().map(|(_, v)| *v).collect(),
            });
        }
    }

    trend
}

fn compute_linear_slope(values: &[(String, f64)]) -> f64 {
    let n = values.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().map(|(_, y)| y).sum::<f64>() / n;

    let numerator: f64 = values
        .iter()
        .enumerate()
        .map(|(i, (_, y))| {
            let x = i as f64;
            (x - x_mean) * (y - y_mean)
        })
        .sum();

    let denominator: f64 = values
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let x = i as f64;
            (x - x_mean).powi(2)
        })
        .sum();

    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

/// Generate Markdown stability report.
pub fn generate_stability_report(record: &StabilityRecord) -> String {
    let mut md = String::new();
    md.push_str(&format!("# Stability Report: {}\n\n", record.test_name));
    md.push_str(&format!("- **Runs analyzed:** {}\n", record.runs_analyzed));
    md.push_str(&format!("- **Flaky rate:** {:.1}%\n", record.flaky_rate * 100.0));
    md.push_str(&format!("- **Consecutive failures:** {}\n", record.consecutive_failures));

    if !record.perf_trend.hard_regressions.is_empty() {
        md.push_str("\n## Hard Regressions\n\n");
        for reg in &record.perf_trend.hard_regressions {
            md.push_str(&format!(
                "- {}: {} = {:.0} (threshold: {:.0})\n",
                reg.run_id, reg.metric, reg.value, reg.threshold
            ));
        }
    }

    if !record.perf_trend.drift_warnings.is_empty() {
        md.push_str("\n## Drift Warnings\n\n");
        for warn in &record.perf_trend.drift_warnings {
            md.push_str(&format!(
                "- **{}**: slope = +{:.1} over {} runs\n",
                warn.metric, warn.slope, warn.runs_window
            ));
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(name: &str, status: TestStatus, duration_ms: u64) -> TestResult {
        TestResult {
            run_id: "r".to_string(),
            test_name: name.to_string(),
            query: "q".to_string(),
            strategy: "Chat".to_string(),
            format_skill: None,
            status,
            answer_text: "".to_string(),
            answer_html: None,
            screenshot_path: None,
            llm_calls: vec![],
            tool_calls: vec![],
            retrieval_hits: None,
            token_usage: None,
            duration_ms,
            timestamp: "".to_string(),
            error_message: None,
            diagnostics: None,
            failure_kind: None,
        }
    }

    #[test]
    fn test_flaky_detection() {
        let runs = vec![
            ("r1".to_string(), vec![make_result("t1", TestStatus::Passed, 1000)]),
            ("r2".to_string(), vec![make_result("t1", TestStatus::Failed, 1000)]),
            ("r3".to_string(), vec![make_result("t1", TestStatus::Passed, 1000)]),
            ("r4".to_string(), vec![make_result("t1", TestStatus::Failed, 1000)]),
            ("r5".to_string(), vec![make_result("t1", TestStatus::Failed, 1000)]),
        ];

        let record = analyze_stability("t1", &runs).unwrap();
        assert_eq!(record.runs_analyzed, 5);
        assert_eq!(record.flaky_rate, 0.6); // 3/5 failed
        assert_eq!(record.consecutive_failures, 2);
    }

    #[test]
    fn test_duration_hard_regression() {
        let runs = vec![
            ("r1".to_string(), vec![make_result("t1", TestStatus::Passed, 1000)]),
            ("r2".to_string(), vec![make_result("t1", TestStatus::Passed, 25_000)]),
        ];

        let record = analyze_stability("t1", &runs).unwrap();
        assert_eq!(record.perf_trend.hard_regressions.len(), 1);
        assert_eq!(record.perf_trend.hard_regressions[0].metric, "duration_ms");
    }

    #[test]
    fn test_linear_slope_computation() {
        let values = vec![
            ("r1".to_string(), 10.0),
            ("r2".to_string(), 20.0),
            ("r3".to_string(), 30.0),
            ("r4".to_string(), 40.0),
            ("r5".to_string(), 50.0),
        ];
        let slope = compute_linear_slope(&values);
        assert!(slope > 0.0);
        // Perfect linear: slope should be ~10
        assert!((slope - 10.0).abs() < 0.001);
    }
}
```

- [ ] **Step 2: Wire up trends command in main.rs**

In the `cli::Commands::Trends` arm, replace the placeholder:

```rust
        cli::Commands::Trends { test_name, runs } => {
            let all_runs = loader::discover_runs(&output_dir);
            let recent_runs: Vec<_> = all_runs.iter().rev().take(runs).cloned().collect();
            let run_results: Vec<_> = recent_runs
                .iter()
                .map(|d| {
                    let run_id = d.file_name().unwrap().to_string_lossy().to_string();
                    (run_id, loader::load_run_results(d))
                })
                .collect();

            if let Some(record) = stability::analyze_stability(&test_name, &run_results) {
                let report = stability::generate_stability_report(&record);
                println!("{}", report);
            } else {
                println!("Not enough data for stability analysis of '{}'.", test_name);
            }
        }
```

- [ ] **Step 3: Add stability module to main.rs**

```rust
mod attribution;
mod baseline;
mod cli;
mod coverage;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;
mod stability;
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p e2e-analyzer stability`
Expected: pass.

Run: `cargo test -p e2e-analyzer`
Expected: all tests pass.

- [ ] **Step 5: Final compilation check**

Run: `cargo clippy -p e2e-analyzer -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "feat(e2e-analyzer): add stability and trending analysis (P2)"
```

---

## Task 12: Final integration and documentation

**Files:**
- Modify: `crates/e2e-analyzer/Cargo.toml` (add description)
- Create: `crates/e2e-analyzer/README.md`

- [ ] **Step 1: Update Cargo.toml with description**

```toml
[package]
name = "e2e-analyzer"
description = "Cross-run E2E test analysis framework for regression detection, failure attribution, coverage governance, and stability trending."
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true
```

- [ ] **Step 2: Create README.md**

```markdown
# e2e-analyzer

Cross-run E2E test analysis framework.

## Commands

```bash
# Phase 1: Diff current run against baseline
cargo run -p e2e-analyzer -- diff --current-run-id e2e_20260528-XXXXXX

# Phase 2: Diagnose failures with attribution
cargo run -p e2e-analyzer -- diagnose --run-id e2e_20260528-XXXXXX

# Phase 3: Coverage matrix (P1)
cargo run -p e2e-analyzer -- coverage --runs 30

# Phase 4: Stability trends (P2)
cargo run -p e2e-analyzer -- trends --test-name chat_simple_conversation_state_machine --runs 20

# Combined report
cargo run -p e2e-analyzer -- report --current-run-id e2e_20260528-XXXXXX

# Baseline management
cargo run -p e2e-analyzer -- baseline promote e2e_20260528-XXXXXX
cargo run -p e2e-analyzer -- baseline show
```

## Exit Codes

- `0`: Pass or soft drift only (review recommended)
- `1`: Hard regression detected (CI gate blocked)
```

- [ ] **Step 3: Commit**

```bash
git add crates/e2e-analyzer/
git commit -m "docs(e2e-analyzer): add README and package metadata"
```

---

## Self-Review

### 1. Spec Coverage

| Spec Section | Task | Status |
|-------------|------|--------|
| 3.1 RunRecord | Task 1 (models.rs) | Covered |
| 3.2 TestFingerprint | Task 4 (fingerprint.rs) | Covered |
| 3.3 DiffEntry | Task 1 (models.rs) | Covered |
| 3.4 AttributionReport | Task 1 (models.rs) + Task 8 (attribution.rs) | Covered |
| 3.5 CoverageGap | Task 1 (models.rs) + Task 10 (coverage.rs) | Covered |
| 3.6 StabilityRecord | Task 1 (models.rs) + Task 11 (stability.rs) | Covered |
| 4.1 Baseline Management | Task 3 (baseline.rs) | Covered (3-tier fallback) |
| 4.2 Diff Dimensions | Tasks 5, 6 (diff.rs) | Covered (all 4 dimensions) |
| 4.3 Output Format | Task 7 (report.rs) | Covered (Markdown + JSON) |
| 5.1 Attribution Categories | Task 8 (attribution.rs) | Covered (priority chain) |
| 5.2 First Anomaly | Task 8 (attribution.rs) | Covered |
| 5.3 Confidence Levels | Task 8 (attribution.rs) | Covered |
| 5.5 Mapping from Diff | Task 8 (attribution.rs) | Covered |
| 6 Coverage Matrix | Task 10 (coverage.rs) | Covered (P1) |
| 7 Stability/Trending | Task 11 (stability.rs) | Covered (P2) |
| 8 CLI Interface | Tasks 1, 9 (cli.rs, main.rs) | Covered (all subcommands) |

### 2. Placeholder Scan

- No "TBD", "TODO", "implement later" found.
- All steps contain actual code.
- All test commands are explicit.
- No "Similar to Task N" shortcuts.

### 3. Type Consistency

- `DiffEntry` fields match across diff.rs, attribution.rs, report.rs.
- `TestResult` fields match between models.rs and loader.rs.
- `AttributionReport` fields match between models.rs and attribution.rs.
- All enum variants use consistent `snake_case` serde renaming.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-28-e2e-analysis-framework.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Each task produces a small, reviewable change.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints for review.

**Which approach?**
