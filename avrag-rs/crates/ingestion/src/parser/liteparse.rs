//! LiteParse integration: probe, text extraction, and IR bridging.

use std::path::Path;

use anyhow::{Context, Result};
use liteparse::{LiteParse, LiteParseConfig as LpEngineConfig};
use liteparse::types::PdfInput;
use tracing::debug;

use super::liteparse_config::LiteParseConfig;
use super::liteparse_ir::{
    liteparse_page_probe_from_pdf_probe, LiteParsePageProbe, LiteParseTextBlock,
};
use super::probe::PdfPageProbeResult;

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

    /// Fast probe: parse and map page signals for routing.
    pub async fn probe(&self, pdf_bytes: &[u8]) -> Result<Vec<LiteParsePageProbe>> {
        let parsed = self
            .engine
            .parse_input(PdfInput::Bytes(pdf_bytes.to_vec()))
            .await
            .context("liteparse probe parse failed")?;

        Ok(parsed
            .pages
            .iter()
            .map(|page| {
                let text_chars: usize = page.text_items.iter().map(|t| t.text.chars().count()).sum();
                LiteParsePageProbe {
                    page_number: page.page_number as u32,
                    extracted_text_chars: text_chars,
                    image_hint_count: 0,
                    table_hint_count: 0,
                    likely_scanned: text_chars < self.config.scanned_page_threshold,
                    readable_ratio: None,
                    bigram_repeat_ratio: None,
                    unique_token_ratio: None,
                    watermark_hit: false,
                    figure_area_ratio: None,
                    non_decorative_image_count: None,
                    table_garbled_ratio: None,
                }
            })
            .collect())
    }

    /// Extract text blocks with bbox for selected pages.
    pub async fn extract_blocks(
        &self,
        pdf_bytes: &[u8],
        pages: &[u32],
    ) -> Result<Vec<LiteParseTextBlock>> {
        let parsed = self
            .engine
            .parse_input(PdfInput::Bytes(pdf_bytes.to_vec()))
            .await
            .context("liteparse extract parse failed")?;

        let page_set: std::collections::HashSet<u32> = pages.iter().copied().collect();
        let mut blocks = Vec::new();

        for page in &parsed.pages {
            let pn = page.page_number as u32;
            if !page_set.is_empty() && !page_set.contains(&pn) {
                continue;
            }

            for item in &page.text_items {
                if item.text.trim().is_empty() {
                    continue;
                }
                blocks.push(LiteParseTextBlock {
                    page: pn,
                    text: item.text.clone(),
                    bbox: text_item_bbox(item),
                    block_type: "paragraph".to_string(),
                });
            }
        }

        debug!(block_count = blocks.len(), "liteparse extracted text blocks");
        Ok(blocks)
    }

    /// Parse a file path (Office→PDF or image→PDF should happen upstream).
    pub async fn parse_file(&self, path: impl AsRef<Path>) -> Result<Vec<LiteParseTextBlock>> {
        let parsed = self
            .engine
            .parse(path.as_ref().to_string_lossy().as_ref())
            .await
            .with_context(|| format!("liteparse parse failed for {}", path.as_ref().display()))?;
        let mut blocks = Vec::new();
        for page in &parsed.pages {
            for item in &page.text_items {
                if item.text.trim().is_empty() {
                    continue;
                }
                blocks.push(LiteParseTextBlock {
                    page: page.page_number as u32,
                    text: item.text.clone(),
                    bbox: text_item_bbox(item),
                    block_type: "paragraph".to_string(),
                });
            }
        }
        Ok(blocks)
    }

    /// Export page dimensions from a parse pass (width/height in PDF points).
    pub async fn page_dimensions(
        &self,
        pdf_bytes: &[u8],
    ) -> Result<std::collections::BTreeMap<u32, (f32, f32)>> {
        let parsed = self
            .engine
            .parse_input(PdfInput::Bytes(pdf_bytes.to_vec()))
            .await
            .context("liteparse page dimensions parse failed")?;
        Ok(parsed
            .pages
            .iter()
            .map(|page| {
                (
                    page.page_number as u32,
                    (page.page_width, page.page_height),
                )
            })
            .collect())
    }

    /// Export parse result as JSON for diagnostics.
    pub async fn parse_json(&self, pdf_bytes: &[u8]) -> Result<serde_json::Value> {
        let parsed = self
            .engine
            .parse_input(PdfInput::Bytes(pdf_bytes.to_vec()))
            .await
            .context("liteparse json export failed")?;
        serde_json::to_value(&parsed.pages).context("serialize liteparse pages failed")
    }
}

fn text_item_bbox(item: &liteparse::TextItem) -> [f32; 4] {
    [item.x, item.y, item.x + item.width, item.y + item.height]
}

impl LiteParsePageProbe {
    /// Convert to legacy probe struct for router compatibility during migration.
    pub fn to_pdf_page_probe(&self) -> PdfPageProbeResult {
        liteparse_page_probe_from_pdf_probe(self)
    }
}
