//! Artifact loader for E2E test run directories.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Load all `TestResult`s from a single run directory.
///
/// Walks immediate subdirectories looking for `meta.json`.  If present the
/// file is deserialized into `TestResult` and optional `llm_calls.jsonl` /
/// `tool_calls.jsonl` are attached.
pub fn load_run_results(run_dir: &Path) -> Vec<crate::models::TestResult> {
    let mut results = Vec::new();

    let entries = match fs::read_dir(run_dir) {
        Ok(e) => e,
        Err(_) => return results,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let meta_path = path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }

        let mut result: crate::models::TestResult = match fs::read_to_string(&meta_path)
            .with_context(|| format!("reading {}", meta_path.display()))
            .and_then(|s| serde_json::from_str(&s).with_context(|| format!("parsing {}", meta_path.display())))
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Warning: failed to load {}: {e:?}", meta_path.display());
                continue;
            }
        };

        let llm_path = path.join("llm_calls.jsonl");
        if llm_path.exists() {
            match load_llm_calls(&llm_path) {
                Ok(calls) => result.llm_calls = calls,
                Err(e) => eprintln!("Warning: failed to load {}: {e:?}", llm_path.display()),
            }
        }

        let tool_path = path.join("tool_calls.jsonl");
        if tool_path.exists() {
            match load_tool_calls(&tool_path) {
                Ok(calls) => result.tool_calls = calls,
                Err(e) => eprintln!("Warning: failed to load {}: {e:?}", tool_path.display()),
            }
        }

        results.push(result);
    }

    results
}

/// Load run-level `metadata.json` if it exists.
pub fn load_run_metadata(run_dir: &Path) -> Option<crate::models::RunMetadata> {
    let path = run_dir.join("metadata.json");
    if !path.exists() {
        return None;
    }

    fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))
        .and_then(|s| serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display())))
        .map_err(|e| {
            eprintln!("Warning: failed to load {}: {e:?}", path.display());
            e
        })
        .ok()
}

/// Discover all run directories under `output_dir`.
///
/// Returns paths whose basename starts with `e2e_`, sorted by modification
/// time (newest last).
pub fn discover_runs(output_dir: &Path) -> Vec<PathBuf> {
    let mut runs = Vec::new();

    let entries = match fs::read_dir(output_dir) {
        Ok(e) => e,
        Err(_) => return runs,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        if name.starts_with("e2e_") {
            runs.push(path);
        }
    }

    runs.sort_by(|a, b| {
        let mt_a = fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mt_b = fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mt_a.cmp(&mt_b)
    });

    runs
}

/// Find a run directory whose basename exactly matches `run_id`.
pub fn find_run_dir(output_dir: &Path, run_id: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(output_dir).ok()?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if path.file_name().and_then(|n| n.to_str()) == Some(run_id) {
            return Some(path);
        }
    }

    None
}

/// Find the most recent run on the given git branch that has at least one
/// passed test.
pub fn find_latest_run_on_branch(output_dir: &Path, branch: &str) -> Option<PathBuf> {
    let runs = discover_runs(output_dir);

    // Iterate newest first.
    for run_dir in runs.iter().rev() {
        let meta = load_run_metadata(run_dir);
        let branch_matches = meta
            .as_ref()
            .and_then(|m| m.git_branch_from_anywhere())
            == Some(branch);

        if !branch_matches {
            continue;
        }

        let results = load_run_results(run_dir);
        let has_pass = results.iter().any(|r| r.status == crate::models::TestStatus::Passed);

        if has_pass {
            return Some(run_dir.clone());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_llm_calls(path: &Path) -> Result<Vec<crate::models::LlmCall>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let mut calls = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let call: crate::models::LlmCall = serde_json::from_str(line)
            .with_context(|| format!("parsing line {} in {}", idx + 1, path.display()))?;
        calls.push(call);
    }

    Ok(calls)
}

fn load_tool_calls(path: &Path) -> Result<Vec<crate::models::ToolCallRecord>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let mut calls = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let call: crate::models::ToolCallRecord = serde_json::from_str(line)
            .with_context(|| format!("parsing line {} in {}", idx + 1, path.display()))?;
        calls.push(call);
    }

    Ok(calls)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_run_results_parses_meta_json() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = tmp.path();

        // Create a test subdirectory with a minimal meta.json.
        let test_dir = run_dir.join("chat__teaching__教我理解_rust_生命周期");
        fs::create_dir_all(&test_dir).unwrap();

        let meta = serde_json::json!({
            "run_id": "e2e_20260101_120000_abc123",
            "test_name": "chat__teaching__教我理解_rust_生命周期",
            "query": "教我理解 rust 生命周期",
            "strategy": "teaching",
            "format_skill": null,
            "status": "passed",
            "answer_text": "生命周期是 Rust 的核心概念...",
            "answer_html": null,
            "screenshot_path": null,
            "llm_calls": [],
            "tool_calls": [],
            "retrieval_hits": null,
            "token_usage": null,
            "duration_ms": 1234,
            "timestamp": "2026-01-01T12:00:00Z",
            "error_message": null,
            "diagnostics": null,
            "failure_kind": null,
        });

        let mut meta_file = fs::File::create(test_dir.join("meta.json")).unwrap();
        meta_file.write_all(meta.to_string().as_bytes()).unwrap();

        // Load and verify.
        let results = load_run_results(run_dir);
        assert_eq!(results.len(), 1);

        let result = &results[0];
        assert_eq!(result.run_id, "e2e_20260101_120000_abc123");
        assert_eq!(result.test_name, "chat__teaching__教我理解_rust_生命周期");
        assert_eq!(result.query, "教我理解 rust 生命周期");
        assert_eq!(result.strategy, "teaching");
        assert_eq!(result.status, crate::models::TestStatus::Passed);
        assert_eq!(result.answer_text, "生命周期是 Rust 的核心概念...");
        assert_eq!(result.duration_ms, 1234);
        assert!(result.llm_calls.is_empty());
        assert!(result.tool_calls.is_empty());
    }
}
