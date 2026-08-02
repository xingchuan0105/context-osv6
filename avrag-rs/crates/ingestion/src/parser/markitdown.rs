//! markitdown 子进程后端（2026-08-02 起为文本/代码类兜底；PDF→liteparse、
//! Office→office-direct，见 `docs/plans/2026-08-02-parser-pipeline-direct-readers.md`）。
//!
//! txt/md/rst/csv/tsv/json/toml/yaml/yml/html/htm/代码扩展名经 markitdown CLI
//! 解析为 markdown，再切 Heading/Paragraph blocks（与 E2E harness
//! `markitdown_reingest.rs` 的切块形状一致——刻意不做管道表重检测，TableIr 退役）。
//!
//! 已知取舍（设计声明）：markitdown 不产多模态 asset（docx/pptx/pdf 内嵌图片不提取）、
//! 不做 OCR（扫描版 PDF 产出近空文本，会被终端零 chunk 完整性检查拒灌）；standalone
//! 图片仍走 PaddleOCR（不经本模块）。
//!
//! 配置：`MARKITDOWN_BIN`（默认 `markitdown`）、`MARKITDOWN_TIMEOUT_MS`
//! （默认 120_000）。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use uuid::Uuid;

use crate::ir::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, MD_LINE_END_KEY,
    MD_LINE_START_KEY, ParseBackend, SourceLocator,
};

/// 子进程调用配置。
#[derive(Debug, Clone)]
pub struct MarkitdownConfig {
    pub bin: String,
    pub timeout: Duration,
}

impl MarkitdownConfig {
    pub fn from_env() -> Self {
        let bin = std::env::var("MARKITDOWN_BIN").unwrap_or_else(|_| "markitdown".to_string());
        let timeout_ms = std::env::var("MARKITDOWN_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120_000);
        Self {
            bin,
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

impl Default for MarkitdownConfig {
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
        .unwrap_or_else(|| "bin".to_string());
    std::env::temp_dir().join(format!("avrag-markitdown-{}.{extension}", Uuid::new_v4()))
}

/// bytes → 临时文件（markitdown 按扩展名选 converter）→ 子进程 → markdown stdout。
pub async fn run_markitdown(
    bytes: &[u8],
    filename: &str,
    config: &MarkitdownConfig,
) -> anyhow::Result<String> {
    let input_path = temp_input_path(filename);
    tokio::fs::write(&input_path, bytes)
        .await
        .map_err(|error| {
            anyhow::anyhow!("markitdown temp file {}: {error}", input_path.display())
        })?;
    let run_result = run_markitdown_on_path(&input_path, config).await;
    let _ = tokio::fs::remove_file(&input_path).await;
    run_result
}

async fn run_markitdown_on_path(
    input_path: &std::path::Path,
    config: &MarkitdownConfig,
) -> anyhow::Result<String> {
    let child = tokio::process::Command::new(&config.bin)
        .arg(input_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            anyhow::anyhow!(
                "markitdown spawn failed (bin {:?}): {error} — worker host 需安装 markitdown CLI",
                config.bin
            )
        })?;
    let output = match tokio::time::timeout(config.timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|error| anyhow::anyhow!("markitdown wait: {error}"))?,
        Err(_) => {
            // kill_on_drop：child 在此作用域结束即回收。
            anyhow::bail!(
                "markitdown timed out after {}ms for {}",
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
            "markitdown exited with {} for {}: {stderr_tail}",
            output.status,
            input_path.display()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// markitdown markdown → Heading/Paragraph blocks（不触发管道表重检测：
/// block_type 只给 Heading/Paragraph；与 `markitdown_reingest.rs` 对齐，
/// parser_backend 一律 [`ParseBackend::Markitdown`]）。
///
/// 每个 block 的 `metadata` 写入 [`MD_LINE_START_KEY`] / [`MD_LINE_END_KEY`]
/// （0-based、闭区间，见 ir.rs 常量注释）：Heading 为所在单行；Paragraph 为其
/// 缓冲行覆盖的区间（含内部空行）。
pub fn blocks_from_markdown(md: &str) -> Vec<BlockIr> {
    let mut blocks: Vec<BlockIr> = Vec::new();
    let mut buf: Vec<&str> = Vec::new();
    // buf 当前覆盖的 0-based md 行区间（闭区间）；buf 为空时取值无意义
    // （空 buf 产出的 block 文本为空，会被 push 的 trim 检查跳过）。
    let mut buf_start = 0usize;
    let mut buf_end = 0usize;
    let mut idx = 0usize;
    let push = |blocks: &mut Vec<BlockIr>,
                idx: &mut usize,
                block_type: BlockType,
                text: String,
                line_range: (usize, usize)| {
        if text.trim().is_empty() {
            return;
        }
        let metadata = BTreeMap::from([
            (MD_LINE_START_KEY.to_string(), line_range.0.to_string()),
            (MD_LINE_END_KEY.to_string(), line_range.1.to_string()),
        ]);
        blocks.push(BlockIr {
            block_id: format!("b{idx}"),
            page: None,
            block_type,
            modality: BlockModality::TextOnly,
            text,
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator::default(),
            parser_backend: ParseBackend::Markitdown,
            metadata,
        });
        *idx += 1;
    };
    for (line_no, line) in md.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let pending = buf.join("\n");
            push(
                &mut blocks,
                &mut idx,
                BlockType::Paragraph,
                pending,
                (buf_start, buf_end),
            );
            buf.clear();
            let heading = trimmed.trim_start_matches('#').trim().to_string();
            push(
                &mut blocks,
                &mut idx,
                BlockType::Heading,
                heading,
                (line_no, line_no),
            );
        } else {
            if buf.is_empty() {
                buf_start = line_no;
            }
            buf_end = line_no;
            buf.push(line);
        }
    }
    let pending = buf.join("\n");
    push(
        &mut blocks,
        &mut idx,
        BlockType::Paragraph,
        pending,
        (buf_start, buf_end),
    );
    blocks
}

/// 文档原件 → markitdown markdown + DocumentIr。
///
/// 返回 markdown 原文：表格阶段（struct-supervision）直接消费这份 md
/// （parity 基准即 markitdown md），调用方决定如何传递。
pub async fn parse_markitdown_document_ir(
    document_id: Uuid,
    filename: &str,
    bytes: &[u8],
) -> anyhow::Result<(DocumentIr, String)> {
    let config = MarkitdownConfig::from_env();
    let markdown = run_markitdown(bytes, filename, &config).await?;
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::from_filename(filename),
        ParseBackend::Markitdown,
    );
    ir.blocks = blocks_from_markdown(&markdown);
    Ok((ir, markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_split_headings_and_paragraphs() {
        let md = "# 标题一\n\n第一段。\n继续第一段。\n\n## 标题二\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let blocks = blocks_from_markdown(md);
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].block_type, BlockType::Heading);
        assert_eq!(blocks[0].text, "标题一");
        assert_eq!(blocks[1].block_type, BlockType::Paragraph);
        assert_eq!(blocks[1].text, "\n第一段。\n继续第一段。\n");
        assert_eq!(blocks[2].block_type, BlockType::Heading);
        assert_eq!(blocks[2].text, "标题二");
        assert_eq!(blocks[3].block_type, BlockType::Paragraph);
        assert!(blocks[3].text.contains("| a | b |"));
        assert!(
            blocks
                .iter()
                .all(|b| b.parser_backend == ParseBackend::Markitdown)
        );
    }

    #[test]
    fn blocks_skip_empty_segments() {
        let md = "# 只有标题\n";
        let blocks = blocks_from_markdown(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Heading);
    }

    #[test]
    fn blocks_carry_md_line_ranges() {
        // 行号（0-based）：0 标题 / 1 空 / 2-3 第一段 / 4 空 / 5 标题 / 6 空 / 7-9 表
        let md = "# 标题一\n\n第一段。\n继续第一段。\n\n## 标题二\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let blocks = blocks_from_markdown(md);
        let range = |b: &BlockIr| {
            (
                b.metadata
                    .get(MD_LINE_START_KEY)
                    .and_then(|v| v.parse::<usize>().ok()),
                b.metadata
                    .get(MD_LINE_END_KEY)
                    .and_then(|v| v.parse::<usize>().ok()),
            )
        };
        assert_eq!(range(&blocks[0]), (Some(0), Some(0)), "heading 单行");
        assert_eq!(
            range(&blocks[1]),
            (Some(1), Some(4)),
            "段落含内部与结尾空行"
        );
        assert_eq!(range(&blocks[2]), (Some(5), Some(5)), "heading 单行");
        assert_eq!(range(&blocks[3]), (Some(6), Some(9)), "管道表段落");
    }

    #[tokio::test]
    async fn subprocess_parses_markdown_passthrough() {
        // markitdown 对 .md 透传；本机未装 CLI 时跳过（部署主机必装）。
        if tokio::process::Command::new("markitdown")
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .is_err()
        {
            eprintln!("markitdown CLI not installed; skipping subprocess test");
            return;
        }
        let (ir, markdown) = parse_markitdown_document_ir(
            Uuid::new_v4(),
            "note.md",
            "# 标题\n\n正文。\n".as_bytes(),
        )
        .await
        .expect("markitdown parse");
        assert!(markdown.contains("标题"));
        assert!(!ir.blocks.is_empty());
        assert_eq!(ir.primary_backend, ParseBackend::Markitdown);
        assert_eq!(ir.doc_type, DocumentType::Text);
    }

    #[tokio::test]
    async fn subprocess_missing_binary_reports_install_hint() {
        let config = MarkitdownConfig {
            bin: "markitdown-definitely-missing-bin".to_string(),
            timeout: Duration::from_millis(5_000),
        };
        let error = run_markitdown(b"x", "note.md", &config)
            .await
            .expect_err("missing binary must fail");
        assert!(error.to_string().contains("markitdown spawn failed"));
    }
}
