use std::collections::BTreeMap;

use async_trait::async_trait;

use super::{DocumentParser, Page, ParsedDocument};

pub struct CodeParser;

const CODE_BLOCK_MARKERS: &[&str] = &[
    "// ---",
    "# ---",
    "## ---",
    "// ===",
    "# ===",
    "## ===",
    "// ---cut---",
    "# ---cut---",
    "```",
    "<!-- ---",
];

const DEFAULT_LINES_PER_CHUNK: usize = 100;

#[async_trait]
impl DocumentParser for CodeParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let content = String::from_utf8_lossy(bytes).to_string();
        let extension = filename.rsplit('.').next().unwrap_or("");

        let chunks = split_code_into_chunks(&content);

        let pages: Vec<Page> = chunks
            .into_iter()
            .enumerate()
            .map(|(idx, chunk_content)| Page {
                number: (idx + 1) as u32,
                content: chunk_content,
                cursor: format!("chunk-{}", idx),
            })
            .collect();

        let mut metadata = BTreeMap::new();
        metadata.insert("source_file".to_string(), filename.to_string());
        metadata.insert("parser".to_string(), "code".to_string());
        metadata.insert("language".to_string(), extension.to_string());

        Ok(ParsedDocument {
            title: filename.to_string(),
            pages,
            metadata,
        })
    }
}

fn split_code_into_chunks(content: &str) -> Vec<String> {
    for marker in CODE_BLOCK_MARKERS {
        if content.contains(marker) {
            let chunks: Vec<String> = content
                .split(marker)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if chunks.len() > 1 {
                return chunks;
            }
        }
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::with_capacity(DEFAULT_LINES_PER_CHUNK);
    let mut current_lines = 0;

    for line in lines {
        current_chunk.push(line);
        current_lines += 1;

        if current_lines >= DEFAULT_LINES_PER_CHUNK {
            let chunk_text = current_chunk.join("\n");
            if !chunk_text.trim().is_empty() {
                chunks.push(chunk_text);
            }
            current_chunk = Vec::with_capacity(DEFAULT_LINES_PER_CHUNK);
            current_lines = 0;
        }
    }

    if current_lines > 0 {
        let chunk_text = current_chunk.join("\n");
        if !chunk_text.trim().is_empty() {
            chunks.push(chunk_text);
        }
    }

    if chunks.is_empty() {
        chunks.push(content.to_string());
    }

    chunks
}
