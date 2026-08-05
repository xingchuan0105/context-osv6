//! Shared markdown CLI shell: temp input, timed spawn, stdout or file output.
//! Dialects (anydoc / markitdown / liteparse) supply argv + postprocess only.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use uuid::Uuid;

/// Build a unique temp path preserving `filename` extension.
pub fn temp_input_path(prefix: &str, filename: &str) -> PathBuf {
    let extension = filename
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty() && *ext != filename)
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "bin".to_string());
    std::env::temp_dir().join(format!("avrag-{prefix}-{}.{extension}", Uuid::new_v4()))
}

/// Spawn `bin` with `args`, wait up to `timeout`. Returns raw `Output` on
/// success; errors on spawn failure, timeout, or non-zero exit (stderr tail).
/// Callers decide whether product is stdout (markitdown) or an out-file (anydoc).
pub async fn run_cli_status<S: AsRef<std::ffi::OsStr>>(
    bin: &str,
    args: &[S],
    timeout: Duration,
    label: &str,
) -> anyhow::Result<Output> {
    let mut cmd = tokio::process::Command::new(bin);
    for a in args {
        cmd.arg(a.as_ref());
    }
    let child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            anyhow::anyhow!("{label} spawn failed (bin {bin:?}): {error}")
        })?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|error| anyhow::anyhow!("{label} wait: {error}"))?,
        Err(_) => {
            anyhow::bail!("{label} timed out after {}ms", timeout.as_millis());
        }
    };
    if !output.status.success() {
        let stderr_tail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(500)
            .collect::<String>();
        anyhow::bail!("{label} exited with {}: {stderr_tail}", output.status);
    }
    Ok(output)
}

/// Spawn CLI and return stdout as UTF-8 lossy (markitdown path).
pub async fn run_cli_capture_stdout<S: AsRef<std::ffi::OsStr>>(
    bin: &str,
    args: &[S],
    timeout: Duration,
    label: &str,
) -> anyhow::Result<String> {
    let output = run_cli_status(bin, args, timeout, label).await?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Write bytes to a temp path (caller must remove), return path.
pub async fn write_temp_input(
    prefix: &str,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<PathBuf> {
    let path = temp_input_path(prefix, filename);
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| anyhow::anyhow!("{prefix} temp file {}: {error}", path.display()))?;
    Ok(path)
}

/// After CLI writes `output_path`, read it as UTF-8 lossy string.
pub async fn read_output_file(path: &Path, label: &str) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| anyhow::anyhow!("{label} read {}: {error}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
