use std::collections::BTreeMap;

use ingestion::ir::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, MD_LINE_END_KEY,
    MD_LINE_START_KEY, ParseBackend, SourceLocator,
};
use uuid::Uuid;

/// Audio extensions owned by the transcription path (W4), not the text index.
pub fn is_audio_file(filename: &str) -> bool {
    matches!(
        ext(filename).as_str(),
        "wav" | "mp3" | "m4a" | "aac" | "ogg" | "flac" | "opus" | "wma" | "amr"
    )
}

/// Pure-Rust parse for md / text / code / csv. `None` for formats that need a
/// subprocess parser (heavy path) or cannot be indexed at all.
pub fn parse_light(document_id: &str, filename: &str, bytes: &[u8]) -> Option<DocumentIr> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let (doc_type, backend, blocks) = match ext(filename).as_str() {
        "md" | "markdown" => (DocumentType::Text, ParseBackend::TextLocal, parse_markdown_blocks(&text)),
        "csv" | "tsv" => (DocumentType::Text, ParseBackend::TextLocal, parse_csv_blocks(&text)),
        "rs" | "py" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "go" | "java" | "c" | "h"
        | "cc" | "cpp" | "hpp" | "cs" | "rb" | "php" | "swift" | "kt" | "scala" | "lua" | "sh"
        | "bash" | "zsh" | "ps1" | "sql" | "html" | "htm" | "css" | "scss" | "vue" | "svelte"
        | "json" | "yaml" | "yml" | "toml" => {
            (DocumentType::Code, ParseBackend::CodeLocal, parse_code_blocks(&text))
        }
        "txt" | "rst" | "ini" | "cfg" | "conf" | "log" => {
            (DocumentType::Text, ParseBackend::TextLocal, parse_text_blocks(&text))
        }
        _ => return None,
    };
    Some(build_ir(document_id, filename, doc_type, backend, blocks))
}

/// Heavy formats via the existing ingestion subprocess parsers (binaries from
/// the B-line environment: `lit` for PDF, `anydoc-extract` for office, else
/// `markitdown`). Spawn/timeout failures surface as errors and become an
/// honest `Unsupported` outcome upstream.
pub async fn parse_heavy(document_id: Uuid, filename: &str, bytes: &[u8]) -> anyhow::Result<DocumentIr> {
    let (ir, _markdown) = match ext(filename).as_str() {
        "pdf" => {
            ingestion::parser::parse_liteparse_pdf_document_ir(document_id, filename, bytes).await?
        }
        "doc" | "docx" | "docm" | "xls" | "xlsx" | "xlsm" | "ods" | "odt" | "rtf" | "epub"
        | "ppt" | "pptx" | "odp" => {
            ingestion::parser::parse_anydoc_document_ir(document_id, filename, bytes).await?
        }
        _ => ingestion::parser::parse_markitdown_document_ir(document_id, filename, bytes).await?,
    };
    Ok(ir)
}

fn ext(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn build_ir(
    document_id: &str,
    filename: &str,
    doc_type: DocumentType,
    backend: ParseBackend,
    blocks: Vec<BlockIr>,
) -> DocumentIr {
    let title = blocks
        .iter()
        .find(|b| b.block_type == BlockType::Heading)
        .map(|b| b.text.clone())
        .unwrap_or_else(|| filename.to_string());
    DocumentIr {
        document_id: document_id.to_string(),
        title,
        doc_type,
        primary_backend: backend,
        backend_version: None,
        language: None,
        metadata: BTreeMap::new(),
        pages: Vec::new(),
        blocks,
        assets: Vec::new(),
        warnings: Vec::new(),
    }
}

fn block(
    index: usize,
    block_type: BlockType,
    text: String,
    line_start: usize,
    line_end: usize,
    backend: ParseBackend,
) -> BlockIr {
    let metadata = BTreeMap::from([
        (MD_LINE_START_KEY.to_string(), line_start.to_string()),
        (MD_LINE_END_KEY.to_string(), line_end.to_string()),
    ]);
    BlockIr {
        block_id: format!("b{index}"),
        page: None,
        block_type,
        modality: BlockModality::TextOnly,
        text,
        alt_text: None,
        asset_refs: Vec::new(),
        caption: None,
        section_path: Vec::new(),
        source_locator: SourceLocator::default(),
        parser_backend: backend,
        metadata,
    }
}

fn push_paragraph(
    blocks: &mut Vec<BlockIr>,
    counter: &mut usize,
    buffer: &mut Vec<&str>,
    start: usize,
    backend: ParseBackend,
) {
    if buffer.is_empty() {
        return;
    }
    let end = start + buffer.len() - 1;
    blocks.push(block(
        *counter,
        BlockType::Paragraph,
        buffer.join("\n"),
        start,
        end,
        backend,
    ));
    *counter += 1;
    buffer.clear();
}

fn parse_markdown_blocks(text: &str) -> Vec<BlockIr> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut counter = 0usize;
    let mut paragraph: Vec<&str> = Vec::new();
    let mut paragraph_start = 0usize;

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if let Some((_, title)) = heading_of(trimmed) {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            blocks.push(block(
                counter,
                BlockType::Heading,
                title.to_string(),
                i,
                i,
                ParseBackend::TextLocal,
            ));
            counter += 1;
            i += 1;
            continue;
        }

        if trimmed.starts_with("```") {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            let language = trimmed.trim_start_matches('`').trim().to_string();
            let start = i + 1;
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim_start().starts_with("```") {
                j += 1;
            }
            let mut b = block(
                counter,
                BlockType::Code,
                lines[start..j].join("\n"),
                start,
                j.saturating_sub(1),
                ParseBackend::TextLocal,
            );
            if !language.is_empty() {
                b.metadata.insert("code_language".to_string(), language);
            }
            blocks.push(b);
            counter += 1;
            i = (j + 1).min(lines.len());
            continue;
        }

        if trimmed.starts_with('|') && i + 1 < lines.len() && is_table_separator(lines[i + 1]) {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            let start = i;
            let mut j = i;
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                j += 1;
            }
            blocks.push(block(
                counter,
                BlockType::Table,
                lines[start..j].join("\n"),
                start,
                j - 1,
                ParseBackend::TextLocal,
            ));
            counter += 1;
            i = j;
            continue;
        }

        if trimmed.starts_with('>') {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            let start = i;
            let mut j = i;
            while j < lines.len() && lines[j].trim_start().starts_with('>') {
                j += 1;
            }
            blocks.push(block(
                counter,
                BlockType::Quote,
                lines[start..j].join("\n"),
                start,
                j - 1,
                ParseBackend::TextLocal,
            ));
            counter += 1;
            i = j;
            continue;
        }

        if is_list_item(trimmed) {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            let start = i;
            let mut j = i;
            while j < lines.len()
                && (is_list_item(lines[j].trim_start())
                    || (j > start && lines[j].starts_with("  ") && !lines[j].trim().is_empty()))
            {
                j += 1;
            }
            blocks.push(block(
                counter,
                BlockType::ListItem,
                lines[start..j].join("\n"),
                start,
                j - 1,
                ParseBackend::TextLocal,
            ));
            counter += 1;
            i = j;
            continue;
        }

        if trimmed.is_empty() {
            push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
            i += 1;
            continue;
        }

        if paragraph.is_empty() {
            paragraph_start = i;
        }
        paragraph.push(line);
        i += 1;
    }
    push_paragraph(&mut blocks, &mut counter, &mut paragraph, paragraph_start, ParseBackend::TextLocal);
    blocks
}

fn heading_of(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    rest.starts_with(' ').then(|| (level, rest.trim()))
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.contains('-')
}

fn is_list_item(line: &str) -> bool {
    let head = line.split_whitespace().next().unwrap_or("");
    matches!(head, "-" | "*" | "+")
        || (head.len() > 1
            && head.chars().next().is_some_and(|c| c.is_ascii_digit())
            && (head.ends_with('.') || head.ends_with(')')))
}

fn parse_text_blocks(text: &str) -> Vec<BlockIr> {
    let mut blocks = Vec::new();
    let mut counter = 0usize;
    let mut buffer: Vec<&str> = Vec::new();
    let mut start = 0usize;
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            push_paragraph(&mut blocks, &mut counter, &mut buffer, start, ParseBackend::TextLocal);
        } else {
            if buffer.is_empty() {
                start = i;
            }
            buffer.push(line);
        }
    }
    push_paragraph(&mut blocks, &mut counter, &mut buffer, start, ParseBackend::TextLocal);
    blocks
}

fn parse_code_blocks(text: &str) -> Vec<BlockIr> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let last_line = text.lines().count().saturating_sub(1);
    vec![block(
        0,
        BlockType::Code,
        text.to_string(),
        0,
        last_line,
        ParseBackend::CodeLocal,
    )]
}

fn parse_csv_blocks(text: &str) -> Vec<BlockIr> {
    const ROWS_PER_BLOCK: usize = 100;
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    for (idx, group) in lines.chunks(ROWS_PER_BLOCK).enumerate() {
        let start = idx * ROWS_PER_BLOCK;
        blocks.push(block(
            idx,
            BlockType::Paragraph,
            group.join("\n"),
            start,
            start + group.len() - 1,
            ParseBackend::TextLocal,
        ));
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_parse_yields_headings_code_and_line_ranges() {
        let md = "# 项目笔记\n\n第一段正文。\n\n```rust\nfn main() {}\n```\n\n- 列表项一\n- 列表项二\n";
        let ir = parse_light("doc-1", "notes.md", md.as_bytes()).unwrap();
        assert_eq!(ir.title, "项目笔记");

        let kinds: Vec<&str> = ir.blocks.iter().map(|b| b.block_type.as_str()).collect();
        assert_eq!(kinds[0], "heading");
        let code = ir.blocks.iter().find(|b| b.block_type == BlockType::Code).unwrap();
        assert_eq!(code.metadata.get("code_language").map(String::as_str), Some("rust"));
        assert_eq!(code.metadata.get(MD_LINE_START_KEY).map(String::as_str), Some("5"));
        let list = ir.blocks.iter().find(|b| b.block_type == BlockType::ListItem).unwrap();
        assert_eq!(list.metadata.get(MD_LINE_END_KEY).map(String::as_str), Some("9"));
    }

    #[test]
    fn light_parse_rejects_heavy_and_audio_extensions() {
        assert!(parse_light("d", "a.pdf", b"x").is_none());
        assert!(parse_light("d", "a.docx", b"x").is_none());
        assert!(is_audio_file("meeting.m4a"));
        assert!(!is_audio_file("meeting.md"));
    }

    #[test]
    fn code_and_csv_light_paths() {
        let code = parse_light("d", "lib.rs", b"fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        assert_eq!(code.blocks[0].block_type.as_str(), "code");
        assert_eq!(code.doc_type, DocumentType::Code);

        let csv = parse_light("d", "data.csv", b"id,name\n1,a\n").unwrap();
        assert_eq!(csv.blocks[0].block_type.as_str(), "paragraph");
        assert_eq!(csv.blocks[0].metadata.get(MD_LINE_START_KEY).map(String::as_str), Some("0"));
    }

    #[test]
    fn chunk_plan_carries_heading_chain_for_light_markdown() {
        let md = "# 顶层\n\n引言。\n\n## 小节\n\n小节正文，提到数据库连接池。\n";
        let ir = parse_light("doc-2", "guide.md", md.as_bytes()).unwrap();
        let plan = ingestion::chunker::build_ir_chunk_plan(&ir, "guide.md", &ingestion::chunker::ChunkPolicy::default());
        assert!(!plan.text_chunks.is_empty());
        assert!(plan.text_chunks.iter().any(|c| c.section_path.contains(&"顶层".to_string())));
        assert!(plan.text_chunks.iter().any(|c| c.text.contains("数据库连接池")));
    }
}
