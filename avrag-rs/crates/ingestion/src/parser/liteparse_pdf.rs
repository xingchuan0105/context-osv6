//! liteparse PDF 子进程后端（2026-08-02 起 PDF 路径，见
//! `docs/plans/2026-08-02-parser-pipeline-direct-readers.md`）。
//!
//! `lit parse <pdf> --format markdown --no-ocr`（PDFium 原生抽取，无 OCR、无 LibreOffice），
//! 输出 markdown → Heading/Paragraph blocks。数字保真/结构/速度实测最优；
//! 不做 OCR（扫描页由扫描检测 → PaddleOCR 链路，见设计 §5.2）。
//!
//! 配置：`LITEPARSE_BIN`（默认 `lit`）、`LITEPARSE_TIMEOUT_MS`（默认 120_000）、
//! `LITEPARSE_SCANNED_MIN_CHARS`（默认 500，见 [`is_scanned_markdown`]）。

use std::path::PathBuf;
use std::time::Duration;

use uuid::Uuid;

use crate::ir::{DocumentIr, DocumentType, ParseBackend};
use crate::parser::markitdown::blocks_from_markdown;

/// 扫描版检测：markdown 非空白字符数低于阈值 → 视为扫描版 PDF（需转 PaddleOCR）。
///
/// 阈值 `LITEPARSE_SCANNED_MIN_CHARS`（默认 500）。代价不对称：
/// - 误报（文本 PDF 被判扫描）→ 多花一次 Paddle job，结果仍正确；
/// - 漏报（真扫描件未 OCR）→ 空 IR → 终端 `EmptyIndex` 死档（不可重试）。
/// 因此阈值偏低偏安全。调用方在 liteparse 路由后判定，命中即切 `paddle_ocr_pdf`。
pub fn is_scanned_markdown(markdown: &str) -> bool {
    let min = std::env::var("LITEPARSE_SCANNED_MIN_CHARS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);
    markdown.chars().filter(|c| !c.is_whitespace()).count() < min
}

/// 子进程调用配置。
#[derive(Debug, Clone)]
pub struct LiteparsePdfConfig {
    pub bin: String,
    pub timeout: Duration,
}

impl LiteparsePdfConfig {
    pub fn from_env() -> Self {
        let bin = std::env::var("LITEPARSE_BIN").unwrap_or_else(|_| "lit".to_string());
        let timeout_ms = std::env::var("LITEPARSE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);
        Self {
            bin,
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

impl Default for LiteparsePdfConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn temp_input_path(filename: &str) -> PathBuf {
    let extension = filename
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty() && *ext != filename)
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "pdf".to_string());
    std::env::temp_dir().join(format!("avrag-liteparse-{}.{extension}", Uuid::new_v4()))
}

/// bytes → 临时文件 → `lit parse <tmp> --format markdown --no-ocr` → stdout markdown。
pub async fn run_liteparse_pdf(
    bytes: &[u8],
    filename: &str,
    config: &LiteparsePdfConfig,
) -> anyhow::Result<String> {
    let input_path = temp_input_path(filename);
    tokio::fs::write(&input_path, bytes)
        .await
        .map_err(|error| {
            anyhow::anyhow!("liteparse temp file {}: {error}", input_path.display())
        })?;
    let run_result = run_liteparse_pdf_on_path(&input_path, config).await;
    let _ = tokio::fs::remove_file(&input_path).await;
    run_result
}

async fn run_liteparse_pdf_on_path(
    input_path: &std::path::Path,
    config: &LiteparsePdfConfig,
) -> anyhow::Result<String> {
    let child = tokio::process::Command::new(&config.bin)
        .arg("parse")
        .arg(input_path)
        .arg("--format")
        .arg("markdown")
        .arg("--no-ocr")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            anyhow::anyhow!(
                "liteparse spawn failed (bin {:?}): {error} — worker host 需安装 lit CLI",
                config.bin
            )
        })?;
    let output = match tokio::time::timeout(config.timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|error| anyhow::anyhow!("liteparse wait: {error}"))?,
        Err(_) => {
            anyhow::bail!(
                "liteparse timed out after {}ms for {}",
                config.timeout.as_millis(),
                input_path.display()
            );
        }
    };
    if !output.status.success() {
        let stderr_tail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(500)
            .collect::<String>();
        anyhow::bail!(
            "liteparse exited with {} for {}: {stderr_tail}",
            output.status,
            input_path.display()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 文档原件 → liteparse markdown + DocumentIr（backend=liteparse_v2_pdf）。
pub async fn parse_liteparse_pdf_document_ir(
    document_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<(DocumentIr, String)> {
    let config = LiteparsePdfConfig::from_env();
    let markdown = run_liteparse_pdf(bytes, filename, &config).await?;
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::from_filename(filename),
        ParseBackend::LiteparseV2Pdf,
    );
    ir.blocks = blocks_from_markdown(&markdown);
    Ok((ir, markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subprocess_missing_binary_reports_install_hint() {
        let config = LiteparsePdfConfig {
            bin: "liteparse-definitely-missing-bin".to_string(),
            timeout: Duration::from_millis(5_000),
        };
        let error = run_liteparse_pdf(b"x", "note.pdf", &config)
            .await
            .expect_err("missing binary must fail");
        assert!(error.to_string().contains("liteparse spawn failed"));
    }

    #[test]
    fn scanned_detection_threshold() {
        // 测试内改动 env 是全局副作用；串行测试环境下用 unsafe 块包裹（Rust 2024 起
        // set_var/remove_var 为 unsafe）。真实 worker 中 env 由部署注入，不冲突。
        unsafe {
            std::env::remove_var("LITEPARSE_SCANNED_MIN_CHARS");
            // 空/近空 markdown → 扫描
            assert!(is_scanned_markdown(""));
            assert!(is_scanned_markdown("  \n  \n"));
            assert!(is_scanned_markdown("扫描件无文本层，仅此一段。"));
            // 富文本 → 非扫描
            assert!(!is_scanned_markdown(&"字".repeat(600)));
            // 阈值可配置：调高后原本"非扫描"的文本被视作扫描
            std::env::set_var("LITEPARSE_SCANNED_MIN_CHARS", "2000");
            assert!(is_scanned_markdown(&"字".repeat(600)));
            std::env::remove_var("LITEPARSE_SCANNED_MIN_CHARS");
        }
    }

    #[tokio::test]
    async fn subprocess_parses_pdf_passthrough() {
        // 需 worker 已安装 lit CLI；未装时跳过（部署主机必装）。
        let bin = std::env::var("LITEPARSE_BIN").unwrap_or_else(|_| "lit".to_string());
        if tokio::process::Command::new(&bin)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_err()
        {
            eprintln!("lit CLI not installed; skipping subprocess test");
            return;
        }
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/smoke.pdf");
        let bytes = std::fs::read(fixture).expect("read smoke.pdf");
        let (ir, markdown) =
            parse_liteparse_pdf_document_ir(Uuid::new_v4(), "smoke.pdf", &bytes)
                .await
                .expect("liteparse parse");
        assert!(
            markdown.contains("Hello liteparse smoke page."),
            "md: {markdown}"
        );
        assert!(!ir.blocks.is_empty());
        assert_eq!(ir.primary_backend, ParseBackend::LiteparseV2Pdf);
    }
}
