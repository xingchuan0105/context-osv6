//! anydoc 子进程后端（2026-08-05 起，见 `docs/plans/2026-08-05-parser-pipeline-anydoc.md`）。
//!
//! 覆盖 anydoc 支持的非 PDF 格式（Office / ODF / RTF / EPUB / CSV 等）：子进程
//! `anydoc-extract <in> <out>` → GFM markdown → Heading/Paragraph blocks。
//! PDF **永不**走本模块（路由锁死 liteparse）。
//!
//! 失败语义：hard-fail（spawn/超时/非零退出/缺产物），**不**降级 markitdown。
//!
//! pptx 族后处理：`strip_pptx_hex_runs`（仅演示文稿扩展名；防源文件 hex 残渣）。
//!
//! 配置：`ANYDOC_BIN`（默认 `anydoc-extract`）、`ANYDOC_TIMEOUT_MS`（默认 120_000）。

use std::path::Path;
use std::time::Duration;

use uuid::Uuid;

use crate::ir::{DocumentIr, DocumentType, ParseBackend};
use crate::parser::markitdown::blocks_from_markdown;
use crate::parser::markdown_cli;

/// 子进程调用配置。
#[derive(Debug, Clone)]
pub struct AnydocConfig {
    pub bin: String,
    pub timeout: Duration,
}

impl AnydocConfig {
    pub fn from_env() -> Self {
        let bin = std::env::var("ANYDOC_BIN").unwrap_or_else(|_| "anydoc-extract".to_string());
        let timeout_ms = std::env::var("ANYDOC_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);
        Self {
            bin,
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

impl Default for AnydocConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// 演示文稿扩展（anydoc 输出后做 hex strip）。
pub fn is_presentation_ext(filename: &str) -> bool {
    filename
        .rsplit('.')
        .next()
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "ppt" | "pps" | "pot" | "pptx" | "pptm" | "ppsx" | "ppsm" | "odp"
            )
        })
        .unwrap_or(false)
}

/// 删除连续 ≥100 个 hex 字符的 run（pptx 源粘贴残渣防御；不用于 docx/xlsx/文本）。
pub fn strip_pptx_hex_runs(md: &str) -> String {
    fn is_hex(b: u8) -> bool {
        b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || (b'A'..=b'F').contains(&b)
    }
    let bytes = md.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if is_hex(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_hex(bytes[i]) {
                i += 1;
            }
            if i - start < 100 {
                out.extend_from_slice(&bytes[start..i]);
            }
            // else drop long hex run
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // only removed ASCII hex runs → still valid UTF-8
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned())
}

/// bytes → 临时输入 → `anydoc-extract <in> <out>` → markdown（pptx 族 strip hex）。
pub async fn run_anydoc(
    bytes: &[u8],
    filename: &str,
    config: &AnydocConfig,
) -> anyhow::Result<String> {
    let input_path = markdown_cli::write_temp_input("anydoc", filename, bytes).await?;
    let output_path = std::env::temp_dir().join(format!("avrag-anydoc-out-{}.md", Uuid::new_v4()));

    let run_result = run_anydoc_on_paths(&input_path, &output_path, config).await;

    let _ = tokio::fs::remove_file(&input_path).await;
    let _ = tokio::fs::remove_file(&output_path).await;

    let mut markdown = run_result?;
    if is_presentation_ext(filename) {
        markdown = strip_pptx_hex_runs(&markdown);
    }
    Ok(markdown)
}

async fn run_anydoc_on_paths(
    input_path: &Path,
    output_path: &Path,
    config: &AnydocConfig,
) -> anyhow::Result<String> {
    // File-based product output: only check process status, then read out file.
    let _ = markdown_cli::run_cli_status(
        &config.bin,
        &[input_path.as_os_str(), output_path.as_os_str()],
        config.timeout,
        "anydoc",
    )
    .await?;
    markdown_cli::read_output_file(output_path, "anydoc").await
}

/// 文档原件 → anydoc markdown + DocumentIr（backend=anydoc）。
pub async fn parse_anydoc_document_ir(
    document_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<(DocumentIr, String)> {
    let config = AnydocConfig::from_env();
    let markdown = run_anydoc(bytes, filename, &config).await?;
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::from_filename(filename),
        ParseBackend::Anydoc,
    );
    ir.blocks = blocks_from_markdown(&markdown);
    for block in &mut ir.blocks {
        block.parser_backend = ParseBackend::Anydoc;
    }
    Ok((ir, markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_ext_detection() {
        assert!(is_presentation_ext("a.pptx"));
        assert!(is_presentation_ext("a.PPTX"));
        assert!(is_presentation_ext("a.ppt"));
        assert!(is_presentation_ext("a.odp"));
        assert!(is_presentation_ext("a.ppsx"));
        assert!(!is_presentation_ext("a.docx"));
        assert!(!is_presentation_ext("a.xlsx"));
        assert!(!is_presentation_ext("a.pdf"));
    }

    #[test]
    fn hex_strip_drops_long_runs_keeps_short() {
        let short = "deadbeef".repeat(10); // 80 chars
        assert!(short.len() < 100);
        let long = "AaBbCcDd".repeat(20); // 160 chars
        assert!(long.len() >= 100);
        let md = format!("before {short} mid {long} after");
        let out = strip_pptx_hex_runs(&md);
        assert!(out.contains(&short), "short kept: {out}");
        assert!(!out.contains(&long), "long dropped: {out}");
        assert!(out.contains("before"));
        assert!(out.contains("after"));
    }

    #[test]
    fn hex_strip_preserves_cjk() {
        let md = "中文标题 ABC 正文";
        assert_eq!(strip_pptx_hex_runs(md), md);
    }

    #[tokio::test]
    async fn subprocess_missing_binary_reports_install_hint() {
        let config = AnydocConfig {
            bin: "anydoc-definitely-missing-bin".to_string(),
            timeout: Duration::from_millis(5_000),
        };
        let error = run_anydoc(b"x", "note.xlsx", &config)
            .await
            .expect_err("missing binary must fail");
        assert!(error.to_string().contains("anydoc spawn failed"));
    }

    #[tokio::test]
    async fn subprocess_parses_xlsx_passthrough() {
        let bin = std::env::var("ANYDOC_BIN").unwrap_or_else(|_| "anydoc-extract".to_string());
        if tokio::process::Command::new(&bin)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_err()
        {
            // anydoc-extract --help exits 2 (usage); still means binary exists if spawn ok.
            // Re-check spawn with no args.
            if tokio::process::Command::new(&bin)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .is_err()
            {
                eprintln!("anydoc-extract not installed; skipping subprocess test");
                return;
            }
        }
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/smoke.xlsx");
        let bytes = std::fs::read(fixture).expect("read smoke.xlsx");
        let (ir, markdown) = parse_anydoc_document_ir(Uuid::new_v4(), "smoke.xlsx", &bytes)
            .await
            .expect("anydoc parse");
        assert!(!markdown.trim().is_empty(), "non-empty markdown");
        assert!(!ir.blocks.is_empty());
        assert_eq!(ir.primary_backend, ParseBackend::Anydoc);
        assert!(
            ir.blocks
                .iter()
                .all(|b| b.parser_backend == ParseBackend::Anydoc)
        );
    }
}
