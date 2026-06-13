use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

const OFFICE_EXTENSIONS: &[&str] = &["doc", "docx", "ppt", "pptx"];

pub fn needs_office_to_pdf(filename: &str) -> bool {
    filename
        .rsplit('.')
        .next()
        .map(|ext| OFFICE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Convert Office documents to PDF via LibreOffice headless (doc/ppt only).
pub async fn maybe_convert_office_to_pdf(
    bytes: &[u8],
    filename: &str,
) -> Result<(Vec<u8>, String)> {
    if !needs_office_to_pdf(filename) {
        return Ok((bytes.to_vec(), filename.to_string()));
    }

    let temp_dir = tempfile::tempdir().context("create temp dir for office conversion")?;
    let input_path = temp_dir.path().join(filename);
    tokio::fs::write(&input_path, bytes)
        .await
        .context("write office input for conversion")?;

    let output = Command::new("libreoffice")
        .args([
            "--headless",
            "--convert-to",
            "pdf",
            "--outdir",
            temp_dir.path().to_str().unwrap_or("."),
            input_path.to_str().unwrap_or("input"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("spawn libreoffice for office→pdf conversion")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("LibreOffice conversion failed for {filename}: {stderr}");
    }

    let pdf_name = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let pdf_path = temp_dir.path().join(format!("{pdf_name}.pdf"));
    if !pdf_path.exists() {
        anyhow::bail!("LibreOffice did not produce output PDF for {filename}");
    }

    let pdf_bytes = tokio::fs::read(&pdf_path)
        .await
        .with_context(|| format!("read converted pdf {}", pdf_path.display()))?;
    Ok((pdf_bytes, format!("{pdf_name}.pdf")))
}
