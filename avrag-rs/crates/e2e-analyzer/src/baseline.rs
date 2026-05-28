//! Baseline management for E2E test runs.
//!
//! Provides a three-tier fallback for resolving a baseline run directory:
//! 1. Persistent `.e2e_baseline` file in the output directory
//! 2. CLI-provided baseline run_id
//! 3. Latest successful run on the same git branch as the current run

use std::fs;
use std::path::{Path, PathBuf};

const BASELINE_FILE: &str = ".e2e_baseline";

/// Read the persistent baseline run_id from `.e2e_baseline` file in output_dir.
pub fn read_persistent_baseline(output_dir: &Path) -> Option<String> {
    let path = output_dir.join(BASELINE_FILE);
    if !path.exists() {
        return None;
    }

    fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Write a run_id as the persistent baseline.
pub fn write_persistent_baseline(output_dir: &Path, run_id: &str) -> std::io::Result<()> {
    let path = output_dir.join(BASELINE_FILE);
    fs::write(&path, run_id)
}

/// Resolve baseline run directory using three-tier fallback:
///
/// 1. `.e2e_baseline` file in output_dir
/// 2. CLI-provided baseline run_id
/// 3. Latest successful run on the same git branch as current_run
///
/// "successful" = has at least one test with status == Passed.
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

    // Tier 2: CLI-provided baseline
    if let Some(run_id) = cli_baseline {
        if let Some(dir) = crate::loader::find_run_dir(output_dir, run_id) {
            return Some(dir);
        }
    }

    // Tier 3: latest successful run on the same branch
    let meta = crate::loader::load_run_metadata(current_run_dir);
    if let Some(branch) = meta.and_then(|m| m.git_branch) {
        if let Some(dir) = crate::loader::find_latest_run_on_branch(output_dir, &branch) {
            // A run should not be its own baseline.
            if dir != current_run_dir {
                return Some(dir);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_run_dir(parent: &Path, run_id: &str, branch: &str, has_pass: bool) -> PathBuf {
        let run_dir = parent.join(run_id);
        fs::create_dir_all(&run_dir).unwrap();

        let meta = serde_json::json!({
            "run_id": run_id,
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "2026-01-01T00:01:00Z",
            "git_sha": null,
            "git_branch": branch,
            "environment": "test",
            "total_tests": 1,
            "passed": if has_pass { 1 } else { 0 },
            "failed": if has_pass { 0 } else { 1 },
            "skipped": 0,
        });

        let mut meta_file = fs::File::create(run_dir.join("metadata.json")).unwrap();
        meta_file.write_all(meta.to_string().as_bytes()).unwrap();

        if has_pass {
            let test_dir = run_dir.join("test_a");
            fs::create_dir_all(&test_dir).unwrap();
            let test_meta = serde_json::json!({
                "run_id": run_id,
                "test_name": "test_a",
                "query": "q",
                "strategy": "s",
                "format_skill": null,
                "status": "passed",
                "answer_text": "",
                "answer_html": null,
                "screenshot_path": null,
                "llm_calls": [],
                "tool_calls": [],
                "retrieval_hits": null,
                "token_usage": null,
                "duration_ms": 100,
                "timestamp": "2026-01-01T00:00:00Z",
                "error_message": null,
                "diagnostics": null,
                "failure_kind": null,
            });
            let mut f = fs::File::create(test_dir.join("meta.json")).unwrap();
            f.write_all(test_meta.to_string().as_bytes()).unwrap();
        }

        run_dir
    }

    #[test]
    fn test_read_write_baseline_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();

        // Initially no baseline file exists.
        assert_eq!(read_persistent_baseline(output_dir), None);

        // Write and read back.
        write_persistent_baseline(output_dir, "e2e_20260101_120000_abc").unwrap();
        assert_eq!(
            read_persistent_baseline(output_dir),
            Some("e2e_20260101_120000_abc".to_string())
        );
    }

    #[test]
    fn test_resolve_baseline_prefers_persistent_file() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();

        let baseline_dir = create_run_dir(output_dir, "e2e_baseline_run", "main", true);
        let current_dir = create_run_dir(output_dir, "e2e_current_run", "main", true);

        // Write the persistent baseline file pointing to baseline_dir.
        write_persistent_baseline(output_dir, "e2e_baseline_run").unwrap();

        // Resolve with no CLI override and current_run = current_dir.
        let resolved = resolve_baseline(output_dir, None, &current_dir);
        assert_eq!(resolved, Some(baseline_dir));
    }

    #[test]
    fn test_resolve_baseline_falls_back_to_cli() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();

        let cli_baseline_dir = create_run_dir(output_dir, "e2e_cli_baseline", "main", true);
        let current_dir = create_run_dir(output_dir, "e2e_current_run", "main", true);

        // No persistent baseline file.
        let resolved = resolve_baseline(output_dir, Some("e2e_cli_baseline"), &current_dir);
        assert_eq!(resolved, Some(cli_baseline_dir));
    }

    #[test]
    fn test_resolve_baseline_falls_back_to_branch_latest() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();

        let older_dir = create_run_dir(output_dir, "e2e_older", "feature-x", true);
        let current_dir = create_run_dir(output_dir, "e2e_current", "feature-x", true);

        // No persistent baseline, no CLI baseline.
        let resolved = resolve_baseline(output_dir, None, &current_dir);
        // Should return older_dir (the latest successful run on the same branch
        // that is NOT the current run).
        assert_eq!(resolved, Some(older_dir));
    }

    #[test]
    fn test_resolve_baseline_skips_self_as_baseline() {
        let tmp = tempfile::tempdir().unwrap();
        let output_dir = tmp.path();

        // Only one run on this branch — the current run itself.
        let current_dir = create_run_dir(output_dir, "e2e_only", "main", true);

        let resolved = resolve_baseline(output_dir, None, &current_dir);
        // No other runs exist, so nothing to use as baseline.
        assert_eq!(resolved, None);
    }
}
