use std::collections::BTreeMap;

use common::ParsedPreviewItem;
use text_splitter::{ChunkConfig, CodeSplitter, MarkdownSplitter, TextSplitter};
use tiktoken_rs::{CoreBPE, cl100k_base_singleton};
use uuid::Uuid;

use crate::ir::{BlockType, DocumentIr, ParseBackend, SourceLocator};
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

    let segments = split_text_segments(content, mode, policy);

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

/// Approximate number of characters per cl100k token. cl100k_base averages
/// roughly 4 chars/token for English text; this is only used to translate the
/// char-based `overlap_chars`/`max_chars` policy into token units so that the
/// token-sized `ChunkConfig` can honour them.
const CHARS_PER_TOKEN: usize = 4;

/// Build a token-based [`ChunkConfig`] that honours `policy.overlap_chars`.
///
/// The capacity (`TARGET_CHUNK_TOKENS`) and the overlap are both measured in
/// *tokens* because the config uses a tokenizer sizer. `policy.overlap_chars`
/// is in characters, so it is converted to tokens with [`CHARS_PER_TOKEN`].
/// `with_overlap` rejects values `>= capacity`, so we clamp to stay below it.
///
/// Uses [`cl100k_base_singleton`] so callers that split many micro-blocks
/// (LiteParse IR) do not re-parse the full BPE vocab per block (~80ms each).
fn token_chunk_config(policy: &ChunkPolicy) -> ChunkConfig<&'static CoreBPE> {
    let tokenizer = cl100k_base_singleton();
    let mut config = ChunkConfig::new(TARGET_CHUNK_TOKENS).with_sizer(tokenizer);

    if policy.overlap_chars > 0 {
        // Overlap shares the same unit (tokens) as the capacity. Convert the
        // char-based policy into tokens, then clamp strictly below capacity.
        let overlap_tokens = policy.overlap_chars / CHARS_PER_TOKEN;
        if overlap_tokens > 0 && overlap_tokens < TARGET_CHUNK_TOKENS {
            // `with_overlap` only errors when overlap >= capacity, which we guard
            // against above, so unwrapping is safe.
            config = config.with_overlap(overlap_tokens).expect(
                "overlap is clamped to be strictly less than the chunk capacity, \
                 so with_overlap cannot fail here",
            );
        }
    }

    config
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
    config: ChunkConfig<&'static CoreBPE>,
) -> Option<CodeSplitter<&'static CoreBPE>> {
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
        let tokenizer = cl100k_base_singleton();
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
                let segments = split_text_segments(&unit.text, mode, policy);

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

#[derive(Debug, Clone)]
pub struct IrTextChunkItem {
    pub text: String,
    pub page: Option<u32>,
    pub cursor: usize,
    pub block_id: String,
    pub block_type: BlockType,
    pub parser_backend: ParseBackend,
    pub source_locator: SourceLocator,
    pub section_path: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct IrMultimodalChunkItem {
    pub chunk_id: String,
    pub block_id: String,
    pub asset_ref: String,
    pub image_path: String,
    pub caption: Option<String>,
    pub summary_text: String,
    pub context_text: String,
    pub page: Option<u32>,
    pub block_type: BlockType,
    pub parser_backend: ParseBackend,
    pub source_locator: SourceLocator,
    pub metadata: BTreeMap<String, String>,
}

pub struct IrChunkPlan {
    pub text_chunks: Vec<IrTextChunkItem>,
    pub multimodal_chunks: Vec<IrMultimodalChunkItem>,
}

pub fn build_ir_chunk_plan(doc: &DocumentIr, filename: &str, policy: &ChunkPolicy) -> IrChunkPlan {
    let mut text_chunks: Vec<IrTextChunkItem> = Vec::new();
    let mut multimodal_chunks: Vec<IrMultimodalChunkItem> = Vec::new();
    let mut cursor = 0usize;
    let mode = split_mode_for_file(filename);
    // T2: nearest preceding Heading chain — fills section_path for blocks
    // that don't carry one themselves (previously empty everywhere).
    let mut heading_chain: Vec<String> = Vec::new();

    for block in &doc.blocks {
        if block.block_type == BlockType::Heading {
            update_heading_chain(&mut heading_chain, block);
        }
        let section_path = if block.section_path.is_empty() {
            heading_chain.clone()
        } else {
            block.section_path.clone()
        };

        // T2: table arm — atomic chunk when the table fits the token budget,
        // row-group chunks with a repeated header row when it doesn't. Never
        // splits mid-row. Falls through to the generic text path when the
        // block carries no validated TableIr payload.
        if block.block_type == BlockType::Table
            && let Some(table) = crate::ir::TableIr::from_block(block)
        {
            for item in table_chunk_items(block, &table, &section_path, &mut cursor) {
                text_chunks.push(item);
            }
            continue;
        }

        if block.block_type.supports_text_chunking() {
            let segments = split_text_segments(&block.text, mode, policy);
            for segment in segments {
                let char_count = segment.chars().count();
                if char_count < policy.min_chars && !text_chunks.is_empty() {
                    if let Some(last) = text_chunks.last_mut() {
                        last.text.push_str("\n\n");
                        last.text.push_str(&segment);
                    }
                    continue;
                }

                text_chunks.push(IrTextChunkItem {
                    text: segment,
                    page: block.page.or(block.source_locator.page),
                    cursor,
                    block_id: block.block_id.clone(),
                    block_type: block.block_type.clone(),
                    parser_backend: block.parser_backend.clone(),
                    source_locator: block.source_locator.clone(),
                    section_path: section_path.clone(),
                    metadata: block.metadata.clone(),
                });
                cursor += 1;
            }
        }

        if block.block_type.supports_multimodal_chunking() {
            let Some(asset_ref) = block.asset_refs.first() else {
                continue;
            };
            let Some(asset) = doc.assets.iter().find(|asset| asset.asset_id == *asset_ref) else {
                continue;
            };

            let summary_text = build_multimodal_summary_text(block);
            let context_text = block
                .alt_text
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| summary_text.clone());

            let mut metadata = block.metadata.clone();
            if block.block_type == BlockType::PageRaster && block.asset_refs.len() > 1 {
                metadata.insert("fusion_asset_refs".to_string(), block.asset_refs.join(","));
                metadata.insert("ingest_route".to_string(), "visual".to_string());
            }

            multimodal_chunks.push(IrMultimodalChunkItem {
                chunk_id: Uuid::new_v4().to_string(),
                block_id: block.block_id.clone(),
                asset_ref: asset.asset_id.clone(),
                image_path: asset.storage_path.clone(),
                caption: block.caption.clone(),
                summary_text,
                context_text,
                page: block.page.or(block.source_locator.page).or(asset.page),
                block_type: block.block_type.clone(),
                parser_backend: block.parser_backend.clone(),
                source_locator: block.source_locator.clone(),
                metadata,
            });
        }
    }

    IrChunkPlan {
        text_chunks,
        multimodal_chunks,
    }
}

fn split_text_segments(text: &str, mode: SplitMode, policy: &ChunkPolicy) -> Vec<String> {
    // Config is cheap when the sizer is the process-wide cl100k singleton.
    let config = token_chunk_config(policy);
    match mode {
        SplitMode::Markdown => MarkdownSplitter::new(config)
            .chunks(text)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        SplitMode::Code(language) => match code_splitter(language, config) {
            Some(splitter) => splitter
                .chunks(text)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            None => TextSplitter::new(token_chunk_config(policy))
                .chunks(text)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        },
        SplitMode::Text => TextSplitter::new(config)
            .chunks(text)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    }
}

/// T2: nearest-preceding-heading chain maintenance. A Heading block with an
/// explicit section_path replaces the chain; otherwise its text pushes a new
/// level (capped at 6 entries).
fn update_heading_chain(chain: &mut Vec<String>, block: &crate::ir::BlockIr) {
    if !block.section_path.is_empty() {
        *chain = block.section_path.clone();
        return;
    }
    let text = block.text.trim();
    if text.is_empty() {
        return;
    }
    chain.push(text.to_string());
    if chain.len() > 6 {
        chain.remove(0);
    }
}

/// T2: chunk a structured table block. Atomic when the serialized table fits
/// the token budget; otherwise row-group chunks — each carries the section
/// path + caption + the REPEATED header row, splits only at row boundaries,
/// with no text overlap between groups. Metadata: `table_index`, `row_range`
/// (1-based data-row indexes, inclusive); block_type stays `Table` so the
/// downstream `chunk_type` column lands "table" unchanged.
fn table_chunk_items(
    block: &crate::ir::BlockIr,
    table: &crate::ir::TableIr,
    section_path: &[String],
    cursor: &mut usize,
) -> Vec<IrTextChunkItem> {
    let tokenizer = cl100k_base_singleton();
    let token_len = |text: &str| tokenizer.encode_ordinary(text).len();

    let section_line = if section_path.is_empty() {
        String::new()
    } else {
        format!("{}\n", section_path.join(" / "))
    };
    // table.to_markdown() already leads with the caption when present.
    let full = format!("{section_line}{}", table.to_markdown());

    let table_index = block.source_locator.table_index.unwrap_or(0);
    let make_item = |text: String, row_range: (u32, u32), cursor: &mut usize| {
        let mut source_locator = block.source_locator.clone();
        source_locator.row_range = Some(row_range);
        let mut metadata = block.metadata.clone();
        metadata.insert("table_index".to_string(), table_index.to_string());
        metadata.insert(
            "row_range".to_string(),
            format!("{}-{}", row_range.0, row_range.1),
        );
        let item = IrTextChunkItem {
            text,
            page: block.page.or(block.source_locator.page),
            cursor: *cursor,
            block_id: block.block_id.clone(),
            block_type: block.block_type.clone(),
            parser_backend: block.parser_backend.clone(),
            source_locator,
            section_path: section_path.to_vec(),
            metadata,
        };
        *cursor += 1;
        item
    };

    // Atomic: fits the budget → one chunk, never split.
    if token_len(&full) <= TARGET_CHUNK_TOKENS {
        let last = table.rows.len().max(1) as u32;
        return vec![make_item(full, (1, last), cursor)];
    }

    // Row groups: repeated header, row-boundary splits, no overlap.
    let caption_line = match &table.caption {
        Some(caption) => format!("{caption}\n"),
        None => String::new(),
    };
    let group_prefix = format!("{section_line}{caption_line}{}", table.header_markdown());
    let mut items = Vec::new();
    let mut group_start = 0usize;
    let mut group_text = group_prefix.clone();
    for (row_idx, _row) in table.rows.iter().enumerate() {
        let row_line = format!("\n{}", table.row_markdown(row_idx));
        let candidate_len = token_len(&format!("{group_text}{row_line}"));
        if group_text != group_prefix && candidate_len > TARGET_CHUNK_TOKENS {
            // Flush the current group (1-based inclusive row range).
            items.push(make_item(
                group_text,
                (group_start as u32 + 1, row_idx as u32),
                cursor,
            ));
            group_text = group_prefix.clone();
            group_start = row_idx;
        }
        group_text.push_str(&row_line);
    }
    items.push(make_item(
        group_text,
        (group_start as u32 + 1, table.rows.len() as u32),
        cursor,
    ));
    items
}

fn build_multimodal_summary_text(block: &crate::ir::BlockIr) -> String {
    [
        block.caption.clone().unwrap_or_default(),
        block.section_path.last().cloned().unwrap_or_default(),
        block.alt_text.clone().unwrap_or_else(|| block.text.clone()),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n")
}

#[cfg(test)]
mod ir_chunk_plan_tests {
    use super::*;
    use crate::ir::{
        AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType,
        ParseBackend, SourceLocator,
    };

    #[test]
    fn build_ir_chunk_plan_splits_text_and_multimodal_routes() {
        let document = DocumentIr {
            document_id: "doc-1".to_string(),
            title: "deck".to_string(),
            doc_type: DocumentType::Pptx,
            primary_backend: ParseBackend::PoiPptx,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: Vec::new(),
            blocks: vec![
                BlockIr {
                    block_id: "slide-1-text".to_string(),
                    page: Some(1),
                    block_type: BlockType::SlideText,
                    modality: BlockModality::TextOnly,
                    text: "Agenda".to_string(),
                    alt_text: None,
                    asset_refs: Vec::new(),
                    caption: None,
                    section_path: vec!["Agenda".to_string()],
                    source_locator: SourceLocator {
                        page: Some(1),
                        slide_index: Some(1),
                        ..SourceLocator::default()
                    },
                    parser_backend: ParseBackend::PoiPptx,
                    metadata: BTreeMap::new(),
                },
                BlockIr {
                    block_id: "slide-1-image".to_string(),
                    page: Some(1),
                    block_type: BlockType::SlideImage,
                    modality: BlockModality::ImageWithContext,
                    text: "Revenue chart".to_string(),
                    alt_text: Some("Q1 revenue chart".to_string()),
                    asset_refs: vec!["asset-1".to_string()],
                    caption: Some("Revenue chart".to_string()),
                    section_path: vec!["Agenda".to_string()],
                    source_locator: SourceLocator {
                        page: Some(1),
                        slide_index: Some(1),
                        ..SourceLocator::default()
                    },
                    parser_backend: ParseBackend::PoiPptx,
                    metadata: BTreeMap::new(),
                },
            ],
            assets: vec![AssetIr {
                asset_id: "asset-1".to_string(),
                page: Some(1),
                asset_kind: AssetKind::SlideRender,
                storage_path: "temporary://slide-1.png".to_string(),
                mime_type: Some("image/png".to_string()),
                width: Some(1280),
                height: Some(720),
                parser_backend: ParseBackend::PoiPptx,
                metadata: BTreeMap::new(),
            }],
            warnings: Vec::new(),
        };

        let plan = build_ir_chunk_plan(&document, "deck.pptx", &ChunkPolicy::default());
        assert_eq!(plan.text_chunks.len(), 1);
        assert_eq!(plan.multimodal_chunks.len(), 1);
        assert_eq!(plan.text_chunks[0].block_type, BlockType::SlideText);
        assert_eq!(plan.multimodal_chunks[0].block_type, BlockType::SlideImage);
        assert_eq!(plan.multimodal_chunks[0].asset_ref, "asset-1");
    }

    // ---- T2: table arm -------------------------------------------------------

    fn table_block(table: &crate::ir::TableIr, block_id: &str) -> BlockIr {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            crate::ir::TableIr::METADATA_KEY.to_string(),
            serde_json::to_string(table).unwrap(),
        );
        BlockIr {
            block_id: block_id.to_string(),
            page: Some(1),
            block_type: BlockType::Table,
            modality: BlockModality::TextOnly,
            text: table.to_markdown(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: table.caption.clone(),
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(1),
                table_index: Some(0),
                ..SourceLocator::default()
            },
            parser_backend: ParseBackend::TextLocal,
            metadata,
        }
    }

    fn doc_with_blocks(blocks: Vec<BlockIr>) -> DocumentIr {
        DocumentIr {
            document_id: "doc-t".to_string(),
            title: "t".to_string(),
            doc_type: DocumentType::Text,
            primary_backend: ParseBackend::TextLocal,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: Vec::new(),
            blocks,
            assets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn make_table(rows: usize, cell_words: usize) -> crate::ir::TableIr {
        crate::ir::TableIr {
            caption: Some("表 1 测试表".to_string()),
            headers: vec!["编号".into(), "名称".into(), "说明".into()],
            rows: (1..=rows)
                .map(|i| {
                    vec![
                        i.to_string(),
                        format!("条目{i}"),
                        (0..cell_words)
                            .map(|w| format!("描述词{w}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ]
                })
                .collect(),
            parse_confidence: crate::ir::TableConfidence::High,
            notes: Vec::new(),
        }
    }

    #[test]
    fn small_table_is_one_atomic_chunk() {
        let table = make_table(3, 3);
        let plan = build_ir_chunk_plan(
            &doc_with_blocks(vec![table_block(&table, "t-1")]),
            "t.txt",
            &ChunkPolicy::default(),
        );
        assert_eq!(plan.text_chunks.len(), 1);
        let chunk = &plan.text_chunks[0];
        assert_eq!(chunk.block_type, BlockType::Table);
        assert!(chunk.text.contains("表 1 测试表"), "caption: {}", chunk.text);
        assert!(chunk.text.contains("|编号|名称|说明|"), "header: {}", chunk.text);
        assert!(chunk.text.contains("|3|条目3|"), "last row: {}", chunk.text);
        assert_eq!(
            chunk.metadata.get("row_range").map(String::as_str),
            Some("1-3")
        );
        assert_eq!(chunk.source_locator.row_range, Some((1, 3)));
    }

    #[test]
    fn big_table_splits_into_row_groups_with_repeated_headers() {
        let tokenizer = cl100k_base_singleton();
        let table = make_table(200, 10);
        let plan = build_ir_chunk_plan(
            &doc_with_blocks(vec![table_block(&table, "t-1")]),
            "t.txt",
            &ChunkPolicy::default(),
        );
        assert!(
            plan.text_chunks.len() > 1,
            "200 fat rows must exceed one chunk: {}",
            plan.text_chunks.len()
        );
        let header_line = "|编号|名称|说明|";
        let mut seen_rows: Vec<usize> = Vec::new();
        for chunk in &plan.text_chunks {
            // Every group repeats the header row and stays within budget.
            assert!(chunk.text.contains(header_line), "{}", chunk.text);
            assert!(chunk.text.contains("表 1 测试表"), "caption: {}", chunk.text);
            assert!(
                tokenizer.encode_ordinary(&chunk.text).len() <= TARGET_CHUNK_TOKENS,
                "chunk over budget"
            );
            // Collect data-row numbers present in this group.
            for line in chunk.text.lines() {
                if let Some(rest) = line.strip_prefix('|')
                    && !line.contains("编号")
                    && !line.contains("---")
                    && let Some(num_str) = rest.split('|').next()
                    && let Ok(n) = num_str.parse::<usize>()
                {
                    seen_rows.push(n);
                }
            }
        }
        // No row is split or duplicated across groups; every row appears once.
        seen_rows.sort_unstable();
        assert_eq!(seen_rows.len(), 200, "rows lost or duplicated");
        assert_eq!(seen_rows[0], 1);
        assert_eq!(seen_rows[199], 200);
        // Groups are contiguous row ranges (split at row edges only).
        assert!(seen_rows.windows(2).all(|w| w[1] == w[0] + 1));
        // row_range metadata matches the group's actual span.
        let first = &plan.text_chunks[0];
        let range = first.metadata.get("row_range").unwrap();
        let (lo, hi) = range.split_once('-').unwrap();
        assert_eq!(lo, "1");
        assert!(hi.parse::<usize>().unwrap() < 200, "first group is not the whole table");
    }

    #[test]
    fn section_path_flows_from_preceding_heading() {
        let heading = BlockIr {
            block_id: "h-1".to_string(),
            page: Some(1),
            block_type: BlockType::Heading,
            modality: BlockModality::TextOnly,
            text: "第三章 营销策略".to_string(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(1),
                ..SourceLocator::default()
            },
            parser_backend: ParseBackend::TextLocal,
            metadata: BTreeMap::new(),
        };
        let table = make_table(2, 2);
        let prose = BlockIr {
            block_id: "p-1".to_string(),
            page: Some(1),
            block_type: BlockType::Paragraph,
            modality: BlockModality::TextOnly,
            text: "这是一段足够长的普通段落文字，用来验证标题链会流入普通块的 section_path。".to_string(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(1),
                ..SourceLocator::default()
            },
            parser_backend: ParseBackend::TextLocal,
            metadata: BTreeMap::new(),
        };
        let plan = build_ir_chunk_plan(
            &doc_with_blocks(vec![heading, table_block(&table, "t-1"), prose]),
            "t.txt",
            &ChunkPolicy::default(),
        );
        let table_chunk = plan
            .text_chunks
            .iter()
            .find(|c| c.block_type == BlockType::Table)
            .expect("table chunk");
        assert!(
            table_chunk.text.contains("第三章 营销策略"),
            "section path in table chunk: {}",
            table_chunk.text
        );
        let prose_chunk = plan
            .text_chunks
            .iter()
            .find(|c| c.block_type == BlockType::Paragraph)
            .expect("prose chunk");
        assert_eq!(prose_chunk.section_path, vec!["第三章 营销策略".to_string()]);
    }

    /// S4 P-Scale (L2-patho filter `patho_scale`): LiteParse ~1.5k micro-paragraphs.
    /// Per-block `cl100k_base()` rebuild used to burn minutes and trip task timeout.
    #[test]
    fn patho_scale_micro_blocks_chunk_plan_under_budget() {
        let blocks: Vec<BlockIr> = (0..1500)
            .map(|i| BlockIr {
                block_id: format!("b-{i}"),
                page: Some((i / 100 + 1) as u32),
                block_type: BlockType::Paragraph,
                modality: BlockModality::TextOnly,
                text: format!("short micro block number {i} with a few words"),
                alt_text: None,
                asset_refs: Vec::new(),
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some((i / 100 + 1) as u32),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::LiteParsePdf,
                metadata: BTreeMap::new(),
            })
            .collect();
        let document = DocumentIr {
            document_id: "doc-micro".to_string(),
            title: "paper".to_string(),
            doc_type: DocumentType::Pdf,
            primary_backend: ParseBackend::LiteParsePdf,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: Vec::new(),
            blocks,
            assets: Vec::new(),
            warnings: Vec::new(),
        };

        let started = std::time::Instant::now();
        let plan = build_ir_chunk_plan(&document, "paper.pdf", &ChunkPolicy::default());
        let elapsed = started.elapsed();
        assert!(
            !plan.text_chunks.is_empty(),
            "expected text chunks from micro-blocks"
        );
        assert!(
            elapsed.as_secs() < 10,
            "build_ir_chunk_plan on 1500 micro-blocks took {:?}; tokenizer must use cl100k singleton",
            elapsed
        );
    }
}
