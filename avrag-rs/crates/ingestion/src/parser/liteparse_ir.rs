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

/// Build partial `DocumentIr` from LiteParse text blocks (A/B routes).
pub fn blocks_to_document_ir(
    document_id: Uuid,
    filename: &str,
    blocks: &[LiteParseTextBlock],
    page_dimensions: &BTreeMap<u32, (f32, f32)>,
) -> DocumentIr {
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

    let mut pages_seen: BTreeMap<u32, usize> = BTreeMap::new();
    for block in blocks {
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
        let block_type = match block.block_type.as_str() {
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

    let mut pages_seen: BTreeMap<u32, usize> = BTreeMap::new();
    for block in blocks {
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
        let block_type = match block.block_type.as_str() {
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
}
