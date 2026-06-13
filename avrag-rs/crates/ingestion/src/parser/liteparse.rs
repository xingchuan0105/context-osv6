//! LiteParse integration: probe, text extraction, and IR bridging.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use liteparse::{LiteParse, LiteParseConfig as LpEngineConfig};
use liteparse::types::PdfInput;
use tracing::debug;

use super::liteparse_config::LiteParseConfig;
use super::liteparse_ir::{LiteParsePageProbe, LiteParseTextBlock};

/// Single LiteParse pass: page probes, dimensions, and text blocks for worker reuse.
#[derive(Debug, Clone)]
pub struct ParsedPdfSnapshot {
    probes: Vec<LiteParsePageProbe>,
    page_dimensions: BTreeMap<u32, (f32, f32)>,
    text_blocks: Vec<LiteParseTextBlock>,
}

impl ParsedPdfSnapshot {
    pub fn probes(&self) -> &[LiteParsePageProbe] {
        &self.probes
    }

    pub fn page_dimensions(&self) -> &BTreeMap<u32, (f32, f32)> {
        &self.page_dimensions
    }

    pub fn text_blocks(&self) -> &[LiteParseTextBlock] {
        &self.text_blocks
    }

    /// Filter cached text blocks to selected pages. Empty `pages` returns all blocks.
    pub fn extract_blocks_for_pages(&self, pages: &[u32]) -> Vec<LiteParseTextBlock> {
        if pages.is_empty() {
            return self.text_blocks.clone();
        }
        let page_set: HashSet<u32> = pages.iter().copied().collect();
        self.text_blocks
            .iter()
            .filter(|block| page_set.contains(&block.page))
            .cloned()
            .collect()
    }
}

/// Service wrapper around the LiteParse engine.
pub struct LiteParseService {
    config: LiteParseConfig,
    engine: LiteParse,
}

impl LiteParseService {
    pub fn new(config: LiteParseConfig) -> Self {
        let mut engine_cfg = LpEngineConfig::default();
        engine_cfg.ocr_enabled = config.ocr_enabled;
        engine_cfg.ocr_server_url = config.ocr_server_url.clone();
        engine_cfg.ocr_language = config.ocr_language.clone();
        engine_cfg.quiet = true;

        Self {
            config,
            engine: LiteParse::new(engine_cfg),
        }
    }

    pub fn from_env() -> Self {
        Self::new(LiteParseConfig::from_env())
    }

    pub fn config(&self) -> &LiteParseConfig {
        &self.config
    }

    /// One parse pass producing probes, dimensions, and text blocks.
    pub async fn parse_pdf_document(&self, pdf_bytes: &[u8]) -> Result<ParsedPdfSnapshot> {
        let parsed = self.parse_input(pdf_bytes).await?;
        Ok(snapshot_from_parse_result(&self.config, &parsed.pages))
    }

    /// Fast probe: parse and map page signals for routing.
    pub async fn probe(&self, pdf_bytes: &[u8]) -> Result<Vec<LiteParsePageProbe>> {
        Ok(self.parse_pdf_document(pdf_bytes).await?.probes)
    }

    /// Extract text blocks with bbox for selected pages.
    pub async fn extract_blocks(
        &self,
        pdf_bytes: &[u8],
        pages: &[u32],
    ) -> Result<Vec<LiteParseTextBlock>> {
        Ok(self
            .parse_pdf_document(pdf_bytes)
            .await?
            .extract_blocks_for_pages(pages))
    }

    /// Parse a file path (Office→PDF or image→PDF should happen upstream).
    pub async fn parse_file(&self, path: impl AsRef<Path>) -> Result<Vec<LiteParseTextBlock>> {
        let parsed = self
            .engine
            .parse(path.as_ref().to_string_lossy().as_ref())
            .await
            .with_context(|| format!("liteparse parse failed for {}", path.as_ref().display()))?;
        Ok(snapshot_from_parse_result(&self.config, &parsed.pages).text_blocks)
    }

    /// Export page dimensions from a parse pass (width/height in PDF points).
    pub async fn page_dimensions(
        &self,
        pdf_bytes: &[u8],
    ) -> Result<BTreeMap<u32, (f32, f32)>> {
        Ok(self.parse_pdf_document(pdf_bytes).await?.page_dimensions)
    }

    async fn parse_input(
        &self,
        pdf_bytes: &[u8],
    ) -> Result<liteparse::ParseResult, anyhow::Error> {
        self.engine
            .parse_input(PdfInput::Bytes(pdf_bytes.to_vec()))
            .await
            .context("liteparse parse failed")
    }
}

fn snapshot_from_parse_result(
    config: &LiteParseConfig,
    pages: &[liteparse::ParsedPage],
) -> ParsedPdfSnapshot {
    let mut probes = Vec::with_capacity(pages.len());
    let mut page_dimensions = BTreeMap::new();
    let mut text_blocks = Vec::new();

    for page in pages {
        let page_number = page.page_number as u32;
        let text_chars: usize = page.text_items.iter().map(|t| t.text.chars().count()).sum();

        probes.push(LiteParsePageProbe {
            page_number,
            extracted_text_chars: text_chars,
            image_hint_count: 0,
            table_hint_count: 0,
            likely_scanned: text_chars < config.scanned_page_threshold,
            readable_ratio: None,
            bigram_repeat_ratio: None,
            unique_token_ratio: None,
            watermark_hit: false,
            figure_area_ratio: None,
            non_decorative_image_count: None,
            table_garbled_ratio: None,
        });
        page_dimensions.insert(page_number, (page.page_width, page.page_height));

        for item in &page.text_items {
            if item.text.trim().is_empty() {
                continue;
            }
            text_blocks.push(LiteParseTextBlock {
                page: page_number,
                text: item.text.clone(),
                bbox: text_item_bbox(item),
                block_type: "paragraph".to_string(),
            });
        }
    }

    debug!(
        page_count = pages.len(),
        block_count = text_blocks.len(),
        "liteparse parsed pdf snapshot"
    );

    ParsedPdfSnapshot {
        probes,
        page_dimensions,
        text_blocks,
    }
}

fn text_item_bbox(item: &liteparse::TextItem) -> [f32; 4] {
    [item.x, item.y, item.x + item.width, item.y + item.height]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn phase0_mini_pdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/spike/fixtures/phase0-mini.pdf")
    }

    #[tokio::test]
    async fn parse_pdf_document_covers_probe_dimensions_and_extract() {
        let path = phase0_mini_pdf();
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).expect("read fixture");
        let service = LiteParseService::from_env();

        let snapshot = service
            .parse_pdf_document(&bytes)
            .await
            .expect("parse pdf snapshot");

        assert!(!snapshot.probes().is_empty());
        assert!(!snapshot.page_dimensions().is_empty());
        assert!(!snapshot.text_blocks().is_empty());

        let all_blocks = snapshot.extract_blocks_for_pages(&[]);
        let page_one = snapshot.extract_blocks_for_pages(&[1]);
        assert!(!all_blocks.is_empty());
        assert!(page_one.iter().all(|block| block.page == 1));
    }

    #[tokio::test]
    async fn hybrid_probe_carries_reusable_snapshot() {
        use super::super::liteparse_probe_bridge::probe_pdf_hybrid;
        use super::super::probe::ParseProbeConfig;

        let path = phase0_mini_pdf();
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).expect("read fixture");
        let config = ParseProbeConfig::from_env();

        let outcome = probe_pdf_hybrid(&bytes, "phase0-mini.pdf", &config).expect("hybrid probe");
        assert!(!outcome.probe_result.pdf_page_probes.is_empty());
        assert!(!outcome.liteparse_snapshot.text_blocks().is_empty());
        assert_eq!(
            outcome.liteparse_snapshot.probes().len(),
            outcome.probe_result.pdf_page_probes.len()
        );
    }
}
