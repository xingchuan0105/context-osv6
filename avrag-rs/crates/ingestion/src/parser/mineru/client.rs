use std::collections::BTreeMap;
use anyhow::{Context, Result};
use reqwest::Client;
use tokio::time::Duration;
use tracing::info;

use super::config::{MineruApiMode, MineruConfig};
use super::fallback::{is_low_value_ocr_document, skipped_ocr_page_unit};
use super::upload::{require_remote_source_url, should_use_remote_extract_v4};
use super::{NormalizedDocument, ParsedUnit};

pub struct MineruClient {
    pub(crate) config: MineruConfig,
    pub(crate) client: Client,
}

impl MineruClient {
    pub fn new(config: MineruConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    pub async fn parse(
        &self,
        bytes: &[u8],
        filename: &str,
        source_url: Option<&str>,
    ) -> Result<NormalizedDocument> {
        info!(filename, mode = ?self.config.api_mode, "Starting MinerU precise parse");
        let result = match self.config.api_mode {
            MineruApiMode::LegacyV1Upload => {
                self.parse_with_page_filter_legacy(bytes, filename, None)
                    .await?
            }
            MineruApiMode::ExtractV4 => {
                if should_use_remote_extract_v4(source_url) {
                    let source_url = require_remote_source_url(source_url, filename)?;
                    self.parse_with_page_filter_v4_remote(filename, &source_url, None, false)
                        .await?
                } else {
                    self.parse_with_page_filter_v4_upload(bytes, filename, None, false)
                        .await?
                }
            }
        };
        info!(filename, mode = ?self.config.api_mode, "MinerU parse completed");
        Ok(result)
    }

    pub async fn parse_pdf_pages(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: &[u32],
        source_url: Option<&str>,
    ) -> Result<NormalizedDocument> {
        if page_numbers.is_empty() {
            anyhow::bail!("MinerU PDF page filter must not be empty");
        }

        let mut units = Vec::new();
        let mut title: Option<String> = None;

        match self.config.api_mode {
            MineruApiMode::LegacyV1Upload => {
                for page_number in page_numbers {
                    let mut page_document = self
                        .parse_with_page_filter_legacy(bytes, filename, Some(&[*page_number]))
                        .await
                        .with_context(|| format!("MinerU OCR failed for page {}", page_number))?;
                    title = title.take().or_else(|| Some(page_document.title.clone()));

                    for unit in &mut page_document.units {
                        unit.page = *page_number;
                    }

                    if page_document.units.is_empty() {
                        page_document.units.push(ParsedUnit::new_text(
                            *page_number,
                            String::new(),
                            "mineru_pdf_ocr".to_string(),
                        ));
                    }

                    units.extend(page_document.units);
                }
            }
            MineruApiMode::ExtractV4 => {
                if should_use_remote_extract_v4(source_url) {
                    let source_url = require_remote_source_url(source_url, filename)?;
                    for page_number in page_numbers {
                        let mut page_document = self
                            .parse_with_page_filter_v4_remote(
                                filename,
                                &source_url,
                                Some(&[*page_number]),
                                true,
                            )
                            .await
                            .with_context(|| {
                                format!("MinerU OCR failed for page {}", page_number)
                            })?;
                        title = title.take().or_else(|| Some(page_document.title.clone()));

                        for unit in &mut page_document.units {
                            unit.page = *page_number;
                        }

                        if !is_low_value_ocr_document(&page_document) {
                            units.extend(page_document.units);
                        } else {
                            info!(
                                filename,
                                page_number, "Skipping low-value MinerU OCR result"
                            );
                            units.push(skipped_ocr_page_unit(*page_number));
                        }
                    }
                } else {
                    let batch_document = self
                        .parse_pdf_pages_v4_upload_batch(bytes, filename, page_numbers)
                        .await?;
                    title = title.take().or_else(|| Some(batch_document.title.clone()));
                    units.extend(batch_document.units);
                }
            }
        }

        Ok(NormalizedDocument {
            title: title.unwrap_or_else(|| filename.to_string()),
            units,
            metadata: BTreeMap::new(),
        })
    }

    pub(crate) fn normalize_result(
        &self,
        filename: &str,
        markdown: String,
        images: Vec<super::figure::ImageInfo>,
    ) -> Result<NormalizedDocument> {
        let blocks = super::layout::markdown_blocks(&markdown);
        let title = super::layout::title_from_blocks(&blocks, filename);
        let mut units = super::layout::text_units_from_blocks(&blocks);
        units.extend(super::figure::figure_units_from_images(&blocks, images));
        Ok(NormalizedDocument {
            title,
            units,
            metadata: BTreeMap::new(),
        })
    }
}
