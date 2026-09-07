//! Audio transcription (F2 front-end) via the standalone `asr-filetrans` CLI.
//!
//! The CLI owns credentials (its `.env` chain points at `avrag-rs/.env`) and
//! intermediates (`asr-filetrans/runs/`), so nothing here ever touches keys
//! and nothing temporary lands in the project directory. The finished
//! `transcript.md` — speaker-labeled, absolute-timeline markdown — is written
//! back into the directory's `transcripts/` and enters the index like any
//! document.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use subtex_store_sqlite::SubtexStore;

const TRANSCRIBE_SCRIPT: &str = "/home/chuan/asr-filetrans/bin/transcribe.sh";
pub const TRANSCRIBE_MODEL: &str = "qwen-audio-3.0-asr-flash-filetrans";
/// Write-back location inside the project directory (the convention can
/// override this later; M1 ships the default).
pub const TRANSCRIPTS_DIR: &str = "transcripts";
/// Single-batch confirmation threshold (PRD F2: >2h reports scale first).
pub const CONFIRM_THRESHOLD_SECS: f64 = 7200.0;
const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(4 * 3600);

/// Audio duration in seconds via ffprobe (WSL self-use has it; `None` when it
/// cannot be determined — the threshold check then treats the file as small).
pub fn audio_duration_secs(path: &Path) -> Option<f64> {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse::<f64>().ok()
}

/// Stable, filesystem-safe run name: resumes reuse the same run directory, so
/// a crash never double-bills (the CLI skips finished stages per run).
pub fn run_name(rel_path: &str, content_hash: &str) -> String {
    let stem = Path::new(rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let sanitized: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("subtex-{sanitized}-{}", &content_hash[..8.min(content_hash.len())])
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscribeOutcome {
    pub transcript_dest: PathBuf,
    pub duration_secs: f64,
}

/// Run the CLI for one audio file and copy `transcript.md` back into
/// `<root>/transcripts/<stem>.md`. Idempotent per (file, content hash).
pub async fn transcribe_file(
    root: &Path,
    rel_path: &str,
    content_hash: &str,
    known_duration: Option<f64>,
) -> Result<TranscribeOutcome> {
    let script =
        std::env::var("ASR_TRANSCRIBE_SCRIPT").unwrap_or_else(|_| TRANSCRIBE_SCRIPT.to_string());
    let rel_path = subtex_core::rel_under_root(root, rel_path)
        .with_context(|| format!("audio path outside root: {rel_path}"))?;
    let abs = root.join(&rel_path);
    if !abs.is_file() {
        bail!("audio file missing: {}", abs.display());
    }
    let stem = Path::new(&rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".to_string());
    let run = run_name(&rel_path, content_hash);

    let mut child = tokio::process::Command::new("bash")
        .arg(&script)
        .arg("run")
        .arg(&abs)
        .args(["--name", &run, "--title", &stem])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn transcribe.sh")?;
    let status = match tokio::time::timeout(TRANSCRIBE_TIMEOUT, child.wait()).await {
        Ok(result) => result.context("wait for transcribe.sh")?,
        Err(_) => {
            let _ = child.kill().await;
            bail!("transcribe.sh timed out after 4h");
        }
    };
    // The CLI prints a bounded number of lines, so reading the pipes after
    // exit cannot deadlock in practice.
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    {
        use tokio::io::AsyncReadExt;
        if let Some(mut out) = child.stdout.take() {
            let _ = out.read_to_end(&mut stdout_buf).await;
        }
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_end(&mut stderr_buf).await;
        }
    }

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_buf);
        let tail: Vec<&str> = stderr.lines().rev().take(3).collect();
        bail!(
            "transcribe.sh failed ({}): {}",
            status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        );
    }

    let stdout = String::from_utf8_lossy(&stdout_buf);
    let transcript_src = stdout
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("TRANSCRIPT=").map(PathBuf::from))
        .with_context(|| format!("transcribe.sh printed no TRANSCRIPT line for {rel_path}"))?;
    if !transcript_src.is_file() {
        bail!("transcript missing after run: {}", transcript_src.display());
    }

    let dest_dir = root.join(TRANSCRIPTS_DIR);
    tokio::fs::create_dir_all(&dest_dir).await?;
    let dest = dest_dir.join(format!("{stem}.md"));
    tokio::fs::copy(&transcript_src, &dest).await
        .with_context(|| format!("write transcript back to {}", dest.display()))?;

    let duration_secs = known_duration.or_else(|| audio_duration_secs(&abs)).unwrap_or(0.0);
    Ok(TranscribeOutcome {
        transcript_dest: dest,
        duration_secs,
    })
}

/// Record one transcription into the store's usage ledger.
pub fn record_transcription_usage(
    store: &SubtexStore,
    duration_secs: f64,
    run: &str,
) -> Result<(), subtex_store_sqlite::StoreError> {
    store.record_usage(
        "transcription",
        Some(TRANSCRIBE_MODEL),
        duration_secs,
        Some("seconds"),
        Some(&serde_json::json!({ "run": run })),
    )
}

/// Enqueue a transcription job for one audio file unless an equivalent job
/// already covers it (active, or done for the same content hash). Jobs start
/// in `needs_confirmation`; the daemon auto-confirms batches within the
/// threshold, larger batches wait for `subtex.transcribe confirm: true`.
pub fn enqueue_transcribe_job(
    store: &SubtexStore,
    root: &Path,
    rel_path: &str,
) -> Result<bool> {
    let rel_path = subtex_core::rel_under_root(root, rel_path)
        .context("audio path outside root")?;
    let scanned = subtex_core::scanner::scan_file(root, &rel_path)
        .context("scan audio file")?
        .ok_or_else(|| anyhow::anyhow!("audio file missing: {rel_path}"))?;
    if let Some(existing) = store.latest_job("transcribe", &rel_path)? {
        let same_hash_done = existing.state == "done"
            && existing
                .payload
                .as_ref()
                .and_then(|p| p.get("hash"))
                .and_then(serde_json::Value::as_str)
                == Some(scanned.content_hash.as_str());
        let active = matches!(
            existing.state.as_str(),
            "pending" | "running" | "needs_confirmation"
        );
        if active || same_hash_done {
            return Ok(false);
        }
    }
    let duration = audio_duration_secs(&root.join(&rel_path));
    store.enqueue_job_in_state(
        "transcribe",
        Some(&rel_path),
        Some(&serde_json::json!({
            "hash": scanned.content_hash,
            "duration_secs": duration,
        })),
        "needs_confirmation",
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_name_is_stable_and_sanitized() {
        assert_eq!(run_name("notes.m4a", "abcdef1234567890"), "subtex-notes-abcdef12");
        // Non-ASCII and spaces collapse to '_' so the run dir is filesystem-safe.
        assert_eq!(
            run_name("会议录音/2026-09-01 会议.m4a", "abcdef1234567890"),
            "subtex-2026-09-01___-abcdef12"
        );
        assert_eq!(run_name("x.wav", "ab"), "subtex-x-ab");
    }

    #[tokio::test]
    async fn transcribe_reports_missing_audio() {
        let dir = tempfile::TempDir::new().unwrap();
        let err = transcribe_file(dir.path(), "nope.m4a", "hash", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("missing"), "{err}");
    }
}
