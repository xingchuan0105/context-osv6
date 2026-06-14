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
            .and_then(|s| {
                serde_json::from_str(&s).with_context(|| format!("parsing {}", meta_path.display()))
            }) {
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
        .and_then(|s| {
            serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display()))
        })
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
        mt_a.cmp(&mt_b).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    runs
}

/// Discover run directories under `output_dir/{bucket}/` (e.g. `llm_real`).
pub fn discover_bucket_runs(output_dir: &Path, bucket: &str) -> Vec<PathBuf> {
    discover_runs(&output_dir.join(bucket))
}

/// Discover runs in legacy flat layout and all known bucket layouts.
pub fn discover_all_runs(output_dir: &Path) -> Vec<PathBuf> {
    let mut runs = discover_runs(output_dir);
    for bucket in ["llm_real", "observability", "failures"] {
        runs.extend(discover_bucket_runs(output_dir, bucket));
    }

    runs.sort_by(|a, b| {
        let mt_a = fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mt_b = fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mt_a.cmp(&mt_b).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for run in runs {
        let key = run
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if seen.insert(key) {
            deduped.push(run);
        }
    }
    deduped
}

/// Find a run directory whose basename exactly matches `run_id`.
///
/// Searches the legacy flat layout (`e2e_output/e2e_*`) and bucket layouts
/// (`e2e_output/{llm_real,observability,failures}/e2e_*`).
pub fn find_run_dir(output_dir: &Path, run_id: &str) -> Option<PathBuf> {
    find_run_dir_in(output_dir, run_id).or_else(|| {
        for bucket in ["llm_real", "observability", "failures"] {
            if let Some(path) = find_run_dir_in(&output_dir.join(bucket), run_id) {
                return Some(path);
            }
        }
        None
    })
}

fn find_run_dir_in(output_dir: &Path, run_id: &str) -> Option<PathBuf> {
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

/// Load all llm_real test artifacts from a single run directory.
pub fn load_llm_real_run(run_dir: &Path) -> Vec<crate::models::LlmRealTestArtifact> {
    let mut artifacts = Vec::new();
    let entries = match fs::read_dir(run_dir) {
        Ok(e) => e,
        Err(_) => return artifacts,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let test_dir = entry.path();
        if !test_dir.is_dir() {
            continue;
        }
        let metadata_path = test_dir.join("metadata.json");
        if !metadata_path.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&metadata_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: failed to read {}: {e}", metadata_path.display());
                continue;
            }
        };
        let meta: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {e}", metadata_path.display());
                continue;
            }
        };
        let test_name = meta
            .get("test_name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                test_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let mut extra = meta
            .get("extra")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(citation_count) = meta.get("citation_count") {
            if let Some(obj) = extra.as_object_mut() {
                obj.entry("citation_count")
                    .or_insert_with(|| citation_count.clone());
            }
        }
        artifacts.push(crate::models::LlmRealTestArtifact {
            run_id: meta
                .get("run_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            test_name,
            agent_type: meta
                .get("agent_type")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            usage: meta.get("usage").cloned(),
            reasoning_delta_count: meta.get("reasoning_delta_count").and_then(|v| v.as_u64()),
            trace_reasoning_count: meta.get("trace_reasoning_count").and_then(|v| v.as_u64()),
            prompt_snapshot_count: meta.get("prompt_snapshot_count").and_then(|v| v.as_u64()),
            reasoning_empty_warning: meta
                .get("reasoning_empty_warning")
                .and_then(|v| v.as_bool()),
            stream_error_with_done: meta
                .get("stream_error_with_done")
                .and_then(|v| v.as_bool())
                .or_else(|| {
                    meta.get("extra")
                        .and_then(|v| v.get("stream_error_with_done"))
                        .and_then(|v| v.as_bool())
                }),
            extra: Some(extra),
        });
    }

    artifacts.sort_by(|a, b| a.test_name.cmp(&b.test_name));
    artifacts
}

/// Load run metadata plus all test results as a single record envelope.
pub fn load_run_record(run_dir: &Path) -> Option<crate::models::RunRecord> {
    let metadata = load_run_metadata(run_dir)?;
    let results = load_run_results(run_dir);
    Some(crate::models::RunRecord { metadata, results })
}

/// Find the most recent run on the given git branch that has at least one
/// passed test. When `commit` is set, only runs on that commit are considered.
/// When `exclude` is set, that run directory is skipped (e.g. the current run).
pub fn find_latest_run_on_branch(
    output_dir: &Path,
    branch: &str,
    commit: Option<&str>,
    exclude: Option<&Path>,
) -> Option<PathBuf> {
    let runs = discover_runs(output_dir);

    // Iterate newest first.
    for run_dir in runs.iter().rev() {
        if exclude.is_some_and(|dir| dir == run_dir) {
            continue;
        }

        let meta = load_run_metadata(run_dir);
        let branch_matches =
            meta.as_ref().and_then(|m| m.git_branch_from_anywhere()) == Some(branch);
        let commit_matches = commit.is_none_or(|expected| {
            meta.as_ref()
                .and_then(|m| m.git_commit_from_anywhere())
                == Some(expected)
        });

        if !branch_matches || !commit_matches {
            continue;
        }

        let results = load_run_results(run_dir);
        let has_pass = results
            .iter()
            .any(|r| r.status == crate::models::TestStatus::Passed)
            || meta
                .as_ref()
                .and_then(|m| m.passed)
                .unwrap_or(0)
                > 0;

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
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

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
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

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

    #[test]
    fn test_load_llm_real_run_parses_metadata_json() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = tmp.path().join("e2e_20260101_120000_abc123");
        let test_dir = run_dir.join("real_llm_rag_document_qa_returns_citation");
        fs::create_dir_all(&test_dir).unwrap();

        let metadata = serde_json::json!({
            "run_id": "e2e_20260101_120000_abc123",
            "test_name": "real_llm_rag_document_qa_returns_citation",
            "agent_type": "rag",
            "reasoning_delta_count": 3,
            "trace_reasoning_count": 2,
            "prompt_snapshot_count": 4,
            "reasoning_empty_warning": false,
            "stream_error_with_done": true,
            "extra": {"document_id": "doc-1"}
        });
        let mut file = fs::File::create(test_dir.join("metadata.json")).unwrap();
        file.write_all(metadata.to_string().as_bytes()).unwrap();

        let artifacts = load_llm_real_run(&run_dir);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(
            artifacts[0].test_name,
            "real_llm_rag_document_qa_returns_citation"
        );
        assert_eq!(artifacts[0].prompt_snapshot_count, Some(4));
        assert_eq!(artifacts[0].trace_reasoning_count, Some(2));
        assert_eq!(artifacts[0].stream_error_with_done, Some(true));
    }

    #[test]
    fn test_find_run_dir_searches_bucket_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path();
        let run_dir = output.join("llm_real").join("e2e_20260101_120000_abc123");
        fs::create_dir_all(&run_dir).unwrap();

        let found = find_run_dir(output, "e2e_20260101_120000_abc123").unwrap();
        assert_eq!(found, run_dir);
    }
}
