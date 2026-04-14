use common::ParsedPreviewItem;
use text_splitter::{ChunkConfig, CodeSplitter, MarkdownSplitter, TextSplitter};
use tiktoken_rs::{CoreBPE, cl100k_base};
use uuid::Uuid;

use crate::parser::{NormalizedDocument, Page, ParsedDocument, ParsedUnitKind};

const TARGET_CHUNK_TOKENS: usize = 512;

#[derive(Debug, Clone)]
pub struct ChunkPolicy {
    pub max_chars: usize,
    pub overlap_chars: usize,
    pub min_chars: usize,
}

impl Default for ChunkPolicy {
    fn default() -> Self {
        Self {
            max_chars: 512,
            overlap_chars: 64,
            min_chars: 32,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SplitMode {
    Text,
    Markdown,
    Code(CodeLanguage),
}

#[derive(Debug, Clone, Copy)]
enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    Java,
}

pub fn build_chunk_items(
    doc: &ParsedDocument,
    filename: &str,
    policy: &ChunkPolicy,
) -> Vec<ParsedPreviewItem> {
    let mut items = Vec::new();
    let mut cursor = 0usize;
    let mode = split_mode_for_file(filename);

    for page in &doc.pages {
        let page_items = chunk_page(page, mode, policy, &mut cursor);
        items.extend(page_items);
    }

    if items.is_empty() {
        items.push(ParsedPreviewItem {
            kind: "paragraph".to_string(),
            text: "Document uploaded but no previewable text was extracted.".to_string(),
            page: 1,
            cursor: 0,
        });
    }

    items
}

fn chunk_page(
    page: &Page,
    mode: SplitMode,
    policy: &ChunkPolicy,
    cursor: &mut usize,
) -> Vec<ParsedPreviewItem> {
    let content = page.content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    let config = token_chunk_config();
    let segments: Vec<String> = match mode {
        SplitMode::Markdown => MarkdownSplitter::new(config)
            .chunks(content)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        SplitMode::Code(language) => match code_splitter(language, config) {
            Some(splitter) => splitter
                .chunks(content)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            None => TextSplitter::new(token_chunk_config())
                .chunks(content)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        },
        SplitMode::Text => TextSplitter::new(config)
            .chunks(content)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    };

    let mut items: Vec<ParsedPreviewItem> = Vec::new();
    for segment in segments {
        let char_count = segment.chars().count();
        if char_count < policy.min_chars && !items.is_empty() {
            if let Some(last) = items.last_mut() {
                last.text.push_str(
                    "

",
                );
                last.text.push_str(&segment);
            }
            continue;
        }

        items.push(ParsedPreviewItem {
            kind: chunk_kind(mode).to_string(),
            text: segment,
            page: page.number as usize,
            cursor: *cursor,
        });
        *cursor += 1;
    }

    items
}

fn token_chunk_config() -> ChunkConfig<CoreBPE> {
    let tokenizer = cl100k_base().expect("cl100k tokenizer should load");
    ChunkConfig::new(TARGET_CHUNK_TOKENS).with_sizer(tokenizer)
}

fn split_mode_for_file(filename: &str) -> SplitMode {
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" => SplitMode::Markdown,
        "rs" => SplitMode::Code(CodeLanguage::Rust),
        "py" => SplitMode::Code(CodeLanguage::Python),
        "js" | "jsx" => SplitMode::Code(CodeLanguage::JavaScript),
        "ts" => SplitMode::Code(CodeLanguage::TypeScript),
        "tsx" => SplitMode::Code(CodeLanguage::Tsx),
        "go" => SplitMode::Code(CodeLanguage::Go),
        "java" => SplitMode::Code(CodeLanguage::Java),
        _ => SplitMode::Text,
    }
}

fn chunk_kind(mode: SplitMode) -> &'static str {
    match mode {
        SplitMode::Markdown => "markdown",
        SplitMode::Code(_) => "code",
        SplitMode::Text => "paragraph",
    }
}

fn code_splitter(
    language: CodeLanguage,
    config: ChunkConfig<CoreBPE>,
) -> Option<CodeSplitter<CoreBPE>> {
    match language {
        CodeLanguage::Rust => CodeSplitter::new(tree_sitter_rust::LANGUAGE, config).ok(),
        CodeLanguage::Python => CodeSplitter::new(tree_sitter_python::LANGUAGE, config).ok(),
        CodeLanguage::JavaScript => {
            CodeSplitter::new(tree_sitter_javascript::LANGUAGE, config).ok()
        }
        CodeLanguage::TypeScript => {
            CodeSplitter::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT, config).ok()
        }
        CodeLanguage::Tsx => CodeSplitter::new(tree_sitter_typescript::LANGUAGE_TSX, config).ok(),
        CodeLanguage::Go => CodeSplitter::new(tree_sitter_go::LANGUAGE, config).ok(),
        CodeLanguage::Java => CodeSplitter::new(tree_sitter_java::LANGUAGE, config).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Page, ParsedDocument};
    use std::collections::BTreeMap;

    fn doc(page_content: &str) -> ParsedDocument {
        ParsedDocument {
            title: "doc".to_string(),
            pages: vec![Page {
                number: 3,
                content: page_content.to_string(),
                cursor: "page-3".to_string(),
            }],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn build_chunk_items_preserves_page_number() {
        let items = build_chunk_items(
            &doc("Alpha

Beta

Gamma"),
            "notes.txt",
            &ChunkPolicy::default(),
        );
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.page == 3));
    }

    #[test]
    fn build_chunk_items_uses_markdown_kind_for_markdown_files() {
        let items = build_chunk_items(
            &doc("# Title

Paragraph"),
            "notes.md",
            &ChunkPolicy::default(),
        );
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.kind == "markdown"));
    }

    #[test]
    fn build_chunk_items_uses_code_kind_for_supported_code_files() {
        let items = build_chunk_items(
            &doc("fn add(a: i32, b: i32) -> i32 { a + b }
fn sub(a: i32, b: i32) -> i32 { a - b }"),
            "lib.rs",
            &ChunkPolicy::default(),
        );
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.kind == "code"));
    }

    #[test]
    fn build_chunk_items_respects_token_budget() {
        let tokenizer = cl100k_base().unwrap();
        let long = (0..5000)
            .map(|i| format!("token{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let items = build_chunk_items(&doc(&long), "notes.txt", &ChunkPolicy::default());
        assert!(!items.is_empty());
        assert!(
            items
                .iter()
                .all(|item| tokenizer.encode_ordinary(&item.text).len() <= TARGET_CHUNK_TOKENS)
        );
    }
}

#[derive(Debug, Clone)]
pub struct TextChunkItem {
    pub text: String,
    pub page: u32,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct MultimodalChunkItem {
    pub chunk_id: String,
    pub asset_id: String,
    pub image_path: String,
    pub caption: Option<String>,
    pub context_text: String,
    pub page: u32,
}

pub struct ChunkPlan {
    pub text_chunks: Vec<TextChunkItem>,
    pub multimodal_chunks: Vec<MultimodalChunkItem>,
}

pub fn build_chunk_plan(
    doc: &NormalizedDocument,
    filename: &str,
    policy: &ChunkPolicy,
) -> ChunkPlan {
    let mut text_chunks: Vec<TextChunkItem> = Vec::new();
    let mut multimodal_chunks = Vec::new();
    let mut cursor = 0usize;
    let mode = split_mode_for_file(filename);

    for unit in &doc.units {
        match unit.kind {
            ParsedUnitKind::Text => {
                let config = token_chunk_config();
                let segments: Vec<String> = match mode {
                    SplitMode::Markdown => MarkdownSplitter::new(config)
                        .chunks(&unit.text)
                        .map(str::trim)
                        .filter(|segment| !segment.is_empty())
                        .map(ToOwned::to_owned)
                        .collect(),
                    SplitMode::Code(language) => match code_splitter(language, config) {
                        Some(splitter) => splitter
                            .chunks(&unit.text)
                            .map(str::trim)
                            .filter(|segment| !segment.is_empty())
                            .map(ToOwned::to_owned)
                            .collect(),
                        None => TextSplitter::new(token_chunk_config())
                            .chunks(&unit.text)
                            .map(str::trim)
                            .filter(|segment| !segment.is_empty())
                            .map(ToOwned::to_owned)
                            .collect(),
                    },
                    SplitMode::Text => TextSplitter::new(config)
                        .chunks(&unit.text)
                        .map(str::trim)
                        .filter(|segment| !segment.is_empty())
                        .map(ToOwned::to_owned)
                        .collect(),
                };

                for segment in segments {
                    let char_count = segment.chars().count();
                    if char_count < policy.min_chars && !text_chunks.is_empty() {
                        if let Some(last) = text_chunks.last_mut() {
                            last.text.push_str("\n\n");
                            last.text.push_str(&segment);
                        }
                        continue;
                    }

                    text_chunks.push(TextChunkItem {
                        text: segment,
                        page: unit.page,
                        cursor,
                    });
                    cursor += 1;
                }
            }
            ParsedUnitKind::ImageWithContext => {
                if let Some(image_path) = &unit.image_path {
                    multimodal_chunks.push(MultimodalChunkItem {
                        chunk_id: Uuid::new_v4().to_string(),
                        asset_id: unit.unit_id.clone(),
                        image_path: image_path.clone(),
                        caption: unit.caption.clone(),
                        context_text: unit
                            .context
                            .clone()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| unit.text.clone()),
                        page: unit.page,
                    });
                }
            }
        }
    }

    ChunkPlan {
        text_chunks,
        multimodal_chunks,
    }
}
