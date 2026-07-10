use std::collections::BTreeMap;

use uuid::Uuid;

use crate::ir::{
    BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
    SourceLocator,
};

/// Per-page probe signals from LiteParse (mirrors [`PdfPageProbeResult`](super::probe::PdfPageProbeResult) + routing fields).
#[derive(Debug, Clone, PartialEq)]
pub struct LiteParsePageProbe {
    pub page_number: u32,
    pub extracted_text_chars: usize,
    pub image_hint_count: usize,
    pub table_hint_count: usize,
    pub likely_scanned: bool,
    pub readable_ratio: Option<f32>,
    pub bigram_repeat_ratio: Option<f32>,
    pub unique_token_ratio: Option<f32>,
    pub watermark_hit: bool,
    pub figure_area_ratio: Option<f32>,
    pub non_decorative_image_count: Option<usize>,
    pub table_garbled_ratio: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiteParseTextBlock {
    pub page: u32,
    pub text: String,
    /// `[x1, y1, x2, y2]` PDF points, origin top-left.
    pub bbox: [f32; 4],
    pub block_type: String,
}

/// Y-center distance (PDF points) treated as the same text line.
const SAME_LINE_Y_TOLERANCE: f32 = 4.0;
/// Max gap between previous bottom and next top to stay in one paragraph.
const PARAGRAPH_LINE_GAP_MAX: f32 = 18.0;

fn normalized_block_type(block_type: &str) -> &str {
    match block_type {
        "heading" => "heading",
        "list_item" => "list_item",
        "table" => "table",
        _ => "paragraph",
    }
}

/// Merge LiteParse micro text runs (glyph/line fragments) into coarser blocks.
///
/// LiteParse often emits one block per short line; without coalescing, IR can
/// reach 1k+ paragraphs and make token-aware chunking disproportionately heavy.
pub fn coalesce_liteparse_text_blocks(blocks: &[LiteParseTextBlock]) -> Vec<LiteParseTextBlock> {
    if blocks.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<LiteParseTextBlock> = blocks
        .iter()
        .filter(|b| !b.text.trim().is_empty())
        .cloned()
        .collect();
    sorted.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then_with(|| {
                a.bbox[1]
                    .partial_cmp(&b.bbox[1])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.bbox[0]
                    .partial_cmp(&b.bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut out: Vec<LiteParseTextBlock> = Vec::with_capacity(sorted.len().min(64));
    for block in sorted {
        let Some(prev) = out.last_mut() else {
            out.push(block);
            continue;
        };

        let prev_kind = normalized_block_type(&prev.block_type);
        let cur_kind = normalized_block_type(&block.block_type);
        if prev.page != block.page || prev_kind != cur_kind {
            out.push(block);
            continue;
        }
        // Keep structural blocks atomic unless they are clearly the same line.
        let prev_y_center = (prev.bbox[1] + prev.bbox[3]) * 0.5;
        let cur_y_center = (block.bbox[1] + block.bbox[3]) * 0.5;
        let same_line = (prev_y_center - cur_y_center).abs() <= SAME_LINE_Y_TOLERANCE;
        if matches!(cur_kind, "heading" | "table") && !same_line {
            out.push(block);
            continue;
        }

        let gap = block.bbox[1] - prev.bbox[3];
        let can_merge = if same_line {
            block.bbox[0] + 1.0 >= prev.bbox[0]
        } else {
            matches!(cur_kind, "paragraph" | "list_item")
                && gap >= -2.0
                && gap <= PARAGRAPH_LINE_GAP_MAX
        };
        if !can_merge {
            out.push(block);
            continue;
        }

        if same_line {
            if !prev.text.ends_with(char::is_whitespace)
                && !block.text.starts_with(char::is_whitespace)
            {
                prev.text.push(' ');
            }
            prev.text.push_str(&block.text);
        } else {
            if !prev.text.ends_with('\n') {
                prev.text.push('\n');
            }
            prev.text.push_str(block.text.trim());
        }
        prev.bbox[0] = prev.bbox[0].min(block.bbox[0]);
        prev.bbox[1] = prev.bbox[1].min(block.bbox[1]);
        prev.bbox[2] = prev.bbox[2].max(block.bbox[2]);
        prev.bbox[3] = prev.bbox[3].max(block.bbox[3]);
    }
    out
}

/// Build partial `DocumentIr` from LiteParse text blocks (A/B routes).
pub fn blocks_to_document_ir(
    document_id: Uuid,
    filename: &str,
    blocks: &[LiteParseTextBlock],
    page_dimensions: &BTreeMap<u32, (f32, f32)>,
) -> DocumentIr {
    let blocks = coalesce_liteparse_text_blocks(blocks);
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename,
        DocumentType::Pdf,
        ParseBackend::LiteParsePdf,
    );
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "liteparse-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "liteparse_hybrid".to_string());
    ir.metadata.insert(
        "liteparse_blocks_coalesced".to_string(),
        "true".to_string(),
    );

    let mut pages_seen: BTreeMap<u32, usize> = BTreeMap::new();
    for block in &blocks {
        *pages_seen.entry(block.page).or_default() += block.text.chars().count();
    }

    for (page_no, char_count) in &pages_seen {
        let (w, h) = page_dimensions
            .get(page_no)
            .copied()
            .unwrap_or((612.0, 792.0));
        let mut meta = BTreeMap::new();
        meta.insert("backend".to_string(), "liteparse_pdf".to_string());
        ir.pages.push(PageIr {
            page_number: *page_no,
            width: Some(w),
            height: Some(h),
            text_char_count: *char_count,
            image_count: 0,
            backend: ParseBackend::LiteParsePdf,
            metadata: meta,
        });
    }

    for (idx, block) in blocks.iter().enumerate() {
        let block_type = match normalized_block_type(&block.block_type) {
            "heading" => BlockType::Heading,
            "list_item" => BlockType::ListItem,
            "table" => BlockType::Table,
            _ => BlockType::Paragraph,
        };
        ir.blocks.push(BlockIr {
            block_id: format!("lp-{}-{}", block.page, idx),
            page: Some(block.page),
            block_type,
            modality: BlockModality::TextOnly,
            text: block.text.clone(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(block.page),
                bbox: Some(block.bbox),
                ..Default::default()
            },
            parser_backend: ParseBackend::LiteParsePdf,
            metadata: BTreeMap::new(),
        });
    }

    ir
}

/// Merge additional LiteParse text blocks into an existing digital IR (budget A-degrade path).
pub fn append_liteparse_blocks_to_ir(ir: &mut DocumentIr, blocks: &[LiteParseTextBlock]) {
    if blocks.is_empty() {
        return;
    }
    let blocks = coalesce_liteparse_text_blocks(blocks);

    let mut pages_seen: BTreeMap<u32, usize> = BTreeMap::new();
    for block in &blocks {
        *pages_seen.entry(block.page).or_default() += block.text.chars().count();
    }

    for (page_no, char_count) in pages_seen {
        if let Some(page) = ir.pages.iter_mut().find(|p| p.page_number == page_no) {
            page.text_char_count = page.text_char_count.saturating_add(char_count);
        } else {
            ir.pages.push(PageIr {
                page_number: page_no,
                width: Some(612.0),
                height: Some(792.0),
                text_char_count: char_count,
                image_count: 0,
                backend: ParseBackend::LiteParsePdf,
                metadata: BTreeMap::from([("backend".to_string(), "liteparse_pdf".to_string())]),
            });
        }
    }

    let base_idx = ir.blocks.len();
    for (offset, block) in blocks.iter().enumerate() {
        let block_type = match normalized_block_type(&block.block_type) {
            "heading" => BlockType::Heading,
            "list_item" => BlockType::ListItem,
            "table" => BlockType::Table,
            _ => BlockType::Paragraph,
        };
        ir.blocks.push(BlockIr {
            block_id: format!("lp-{}-{}", block.page, base_idx + offset),
            page: Some(block.page),
            block_type,
            modality: BlockModality::TextOnly,
            text: block.text.clone(),
            alt_text: None,
            asset_refs: Vec::new(),
            caption: None,
            section_path: Vec::new(),
            source_locator: SourceLocator {
                page: Some(block.page),
                bbox: Some(block.bbox),
                ..Default::default()
            },
            parser_backend: ParseBackend::LiteParsePdf,
            metadata: BTreeMap::new(),
        });
    }
}

pub fn page_has_searchable_text(ir: &DocumentIr, page_number: u32) -> bool {
    ir.blocks
        .iter()
        .any(|block| block.page == Some(page_number) && !block.text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_preserved_in_blocks() {
        let blocks = vec![LiteParseTextBlock {
            page: 1,
            text: "hello".to_string(),
            bbox: [10.0, 20.0, 100.0, 40.0],
            block_type: "paragraph".to_string(),
        }];
        let ir = blocks_to_document_ir(
            Uuid::nil(),
            "test.pdf",
            &blocks,
            &BTreeMap::from([(1, (612.0, 792.0))]),
        );
        assert_eq!(ir.blocks.len(), 1);
        assert_eq!(
            ir.blocks[0].source_locator.bbox,
            Some([10.0, 20.0, 100.0, 40.0])
        );
        assert_eq!(ir.primary_backend, ParseBackend::LiteParsePdf);
    }

    #[test]
    fn coalesce_merges_adjacent_paragraph_lines() {
        let blocks: Vec<LiteParseTextBlock> = (0..20)
            .map(|i| {
                let y = 100.0 + i as f32 * 12.0;
                LiteParseTextBlock {
                    page: 1,
                    text: format!("line{i}"),
                    bbox: [72.0, y, 200.0, y + 10.0],
                    block_type: "paragraph".to_string(),
                }
            })
            .collect();
        let coalesced = coalesce_liteparse_text_blocks(&blocks);
        assert_eq!(coalesced.len(), 1, "expected single paragraph after coalesce");
        assert!(coalesced[0].text.contains("line0"));
        assert!(coalesced[0].text.contains("line19"));

        let ir = blocks_to_document_ir(
            Uuid::nil(),
            "paper.pdf",
            &blocks,
            &BTreeMap::from([(1, (612.0, 792.0))]),
        );
        assert_eq!(ir.blocks.len(), 1);
    }

    #[test]
    fn coalesce_keeps_heading_separate_from_body() {
        let blocks = vec![
            LiteParseTextBlock {
                page: 1,
                text: "Title".to_string(),
                bbox: [72.0, 72.0, 200.0, 90.0],
                block_type: "heading".to_string(),
            },
            LiteParseTextBlock {
                page: 1,
                text: "Body".to_string(),
                bbox: [72.0, 100.0, 200.0, 112.0],
                block_type: "paragraph".to_string(),
            },
            LiteParseTextBlock {
                page: 1,
                text: " more".to_string(),
                bbox: [72.0, 114.0, 200.0, 126.0],
                block_type: "paragraph".to_string(),
            },
        ];
        let coalesced = coalesce_liteparse_text_blocks(&blocks);
        assert_eq!(coalesced.len(), 2);
        assert_eq!(coalesced[0].block_type, "heading");
        assert!(coalesced[1].text.contains("Body"));
        assert!(coalesced[1].text.contains("more"));
    }
}
