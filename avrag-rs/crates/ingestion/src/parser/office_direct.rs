//! office-direct 子进程后端（2026-08-02 起，见 `docs/plans/2026-08-02-parser-pipeline-direct-readers.md`）。
//!
//! docx/xlsx/pptx 由 Python 直读库（mammoth/openpyxl/python-pptx）解析；
//! 旧二进制 doc/ppt/xls 由脚本内部经 soffice **无损转 OOXML** 后再直读
//! （不做 PDF 渲染——避免丢列丢行）。输出 markdown → Heading/Paragraph blocks。
//!
//! 失败语义：脚本 hard-fail（转换失败/超时/产物缺失 → 非零退出 + stderr 错误），
//! 本模块透传为 parse 错误，**不降级回 markitdown**（对 doc/ppt/xls 是乱码）。
//!
//! soffice 并发约束：doc/ppt/xls 路径会拉起 LibreOffice（~600MB/实例 + profile 锁），
//! 用静态信号量串行化（`OFFICE_SOFFICE_MAX_CONCURRENT`，默认 1）。docx/xlsx/pptx
//! 不碰 soffice，不占信号量。
//!
//! 配置：`OFFICE_DIRECT_BIN`（默认 `office-direct-extract`，console-script）、
//! `OFFICE_DIRECT_TIMEOUT_MS`（默认 120_000）、`OFFICE_SOFFICE_MAX_CONCURRENT`（默认 1）。

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::ir::{DocumentIr, DocumentType, ParseBackend};
use crate::parser::markitdown::blocks_from_markdown;

/// 子进程调用配置。
#[derive(Debug, Clone)]
pub struct OfficeDirectConfig {
    pub bin: String,
    pub timeout: Duration,
}

impl OfficeDirectConfig {
    pub fn from_env() -> Self {
        let bin = std::env::var("OFFICE_DIRECT_BIN")
            .unwrap_or_else(|_| "office-direct-extract".to_string());
        let timeout_ms = std::env::var("OFFICE_DIRECT_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);
        Self {
            bin,
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

impl Default for OfficeDirectConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// 旧二进制格式（走 soffice 转换，需信号量串行）。
fn uses_soffice(filename: &str) -> bool {
    filename
        .rsplit('.')
        .next()
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "doc" | "ppt" | "xls"))
        .unwrap_or(false)
}

fn soffice_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| {
        let permits = std::env::var("OFFICE_SOFFICE_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1);
        Semaphore::new(permits.max(1))
    })
}

fn temp_input_path(filename: &str) -> PathBuf {
    let extension = filename
        .rsplit('.')
        .next()
        .filter(|ext| !ext.is_empty() && *ext != filename)
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "bin".to_string());
    std::env::temp_dir().join(format!("avrag-office-direct-{}.{extension}", Uuid::new_v4()))
}

/// bytes → 临时输入文件 → `office-direct-extract <in> <out>` → 读取输出 markdown。
///
/// doc/ppt/xls 路径先取 soffice 信号量 permit（串行化 LibreOffice），否则直接跑。
pub async fn run_office_direct(
    bytes: &[u8],
    filename: &str,
    config: &OfficeDirectConfig,
) -> anyhow::Result<String> {
    let input_path = temp_input_path(filename);
    let output_path = std::env::temp_dir().join(format!("avrag-office-direct-out-{}.md", Uuid::new_v4()));
    tokio::fs::write(&input_path, bytes)
        .await
        .map_err(|error| {
            anyhow::anyhow!("office-direct temp file {}: {error}", input_path.display())
        })?;

    let soffice_gate = if uses_soffice(filename) {
        let permit = soffice_semaphore().acquire().await;
        if let Err(error) = permit {
            let _ = tokio::fs::remove_file(&input_path).await;
            let _ = tokio::fs::remove_file(&output_path).await;
            anyhow::bail!("office-direct soffice semaphore: {error}");
        }
        Some(permit)
    } else {
        None
    };

    let run_result = run_office_direct_on_paths(&input_path, &output_path, config).await;

    drop(soffice_gate); // 释放 soffice 信号量 permit
    let _ = tokio::fs::remove_file(&input_path).await;
    let _ = tokio::fs::remove_file(&output_path).await;
    run_result
}

async fn run_office_direct_on_paths(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    config: &OfficeDirectConfig,
) -> anyhow::Result<String> {
    let child = tokio::process::Command::new(&config.bin)
        .arg(input_path)
        .arg(output_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            anyhow::anyhow!(
                "office-direct spawn failed (bin {:?}): {error} — worker 需安装 office-direct-extract",
                config.bin
            )
        })?;
    let output = match tokio::time::timeout(config.timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|error| anyhow::anyhow!("office-direct wait: {error}"))?,
        Err(_) => {
            anyhow::bail!(
                "office-direct timed out after {}ms for {}",
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
            "office-direct exited with {} for {}: {stderr_tail}",
            output.status,
            input_path.display()
        );
    }
    tokio::fs::read_to_string(output_path)
        .await
        .map_err(|error| anyhow::anyhow!("office-direct read output {}: {error}", output_path.display()))
}

/// 文档原件 → office-direct markdown + DocumentIr（backend=office_direct）。
pub async fn parse_office_direct_document_ir(
    document_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<(DocumentIr, String)> {
    let config = OfficeDirectConfig::from_env();
    let markdown = run_office_direct(bytes, filename, &config).await?;
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::from_filename(filename),
        ParseBackend::OfficeDirect,
    );
    ir.blocks = blocks_from_markdown(&markdown);
    Ok((ir, markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subprocess_missing_binary_reports_install_hint() {
        let config = OfficeDirectConfig {
            bin: "office-direct-definitely-missing-bin".to_string(),
            timeout: Duration::from_millis(5_000),
        };
        let error = run_office_direct(b"x", "note.xlsx", &config)
            .await
            .expect_err("missing binary must fail");
        assert!(error.to_string().contains("office-direct spawn failed"));
    }

    #[test]
    fn soffice_gate_only_for_binary_formats() {
        assert!(uses_soffice("a.doc"));
        assert!(uses_soffice("a.ppt"));
        assert!(uses_soffice("a.xls"));
        assert!(!uses_soffice("a.docx"));
        assert!(!uses_soffice("a.pptx"));
        assert!(!uses_soffice("a.xlsx"));
        assert!(!uses_soffice("a.pdf"));
    }

    #[tokio::test]
    async fn subprocess_parses_xlsx_passthrough() {
        // 需 worker 已安装 office-direct-extract；未装时跳过（部署主机必装）。
        let bin = std::env::var("OFFICE_DIRECT_BIN")
            .unwrap_or_else(|_| "office-direct-extract".to_string());
        if tokio::process::Command::new(&bin)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_err()
        {
            eprintln!("office-direct-extract not installed; skipping subprocess test");
            return;
        }
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/smoke.xlsx");
        let bytes = std::fs::read(fixture).expect("read smoke.xlsx");
        let (ir, markdown) =
            parse_office_direct_document_ir(Uuid::new_v4(), "smoke.xlsx", &bytes)
                .await
                .expect("office-direct parse");
        assert!(markdown.contains("Sheet1"), "sheet heading: {markdown}");
        assert!(markdown.contains("甲"), "cell value: {markdown}");
        assert!(!ir.blocks.is_empty());
        assert_eq!(ir.primary_backend, ParseBackend::OfficeDirect);
    }
}
