use std::collections::BTreeMap;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::Duration;

#[derive(Debug, Clone)]
pub struct PdfRendererServiceConfig {
    pub base_url: String,
    pub timeout_ms: u64,
}

impl PdfRendererServiceConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("PDF_RENDERER_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9091".to_string());
        if base_url.trim().is_empty() {
            return None;
        }
        let timeout_ms = std::env::var("PDF_RENDERER_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60_000);
        Some(Self {
            base_url,
            timeout_ms,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenderedPdfPage {
    pub page_number: u32,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub image_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenderPagesResponse {
    pub pages: Vec<RenderedPdfPage>,
}

#[derive(Debug, Clone)]
pub struct PdfRendererServiceClient {
    config: PdfRendererServiceConfig,
    client: Client,
}

impl PdfRendererServiceClient {
    pub fn new(config: PdfRendererServiceConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("pdf renderer reqwest client should build");
        Self { config, client }
    }

    pub async fn healthz(&self) -> Result<bool> {
        let url = format!(
            "{}/v1/healthz",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self.client.get(url).send().await?;
        Ok(response.status().is_success())
    }

    pub async fn render_pages(
        &self,
        pdf_bytes: &[u8],
        filename: &str,
        page_start: u32,
        page_end: u32,
        strategy: &str,
    ) -> Result<RenderPagesResponse> {
        let url = format!(
            "{}/v1/render-pages",
            self.config.base_url.trim_end_matches('/')
        );
        let part = reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("application/pdf")
            .context("pdf multipart mime")?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("page_start", page_start.to_string())
            .text("page_end", page_end.to_string())
            .text("strategy", strategy.to_string());

        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .context("pdf renderer request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("pdf renderer error {status}: {body}");
        }

        response
            .json::<RenderPagesResponse>()
            .await
            .context("parse pdf renderer response")
    }
}

pub fn pages_per_visual_chunk() -> u32 {
    std::env::var("PDF_VISUAL_PAGES_PER_CHUNK")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= 1 && *value <= 5)
        .unwrap_or(4)
}

pub fn visual_render_strategy() -> String {
    std::env::var("PDF_VISUAL_RENDER_STRATEGY")
        .unwrap_or_else(|_| "pixmap_72dpi".to_string())
}

pub fn chunk_page_ranges(pages: &[u32], chunk_size: u32) -> Vec<(u32, u32)> {
    if pages.is_empty() {
        return Vec::new();
    }
    let mut sorted = pages.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut ranges = Vec::new();
    let mut start = sorted[0];
    let mut prev = sorted[0];
    for page in sorted.into_iter().skip(1) {
        if page == prev + 1 && page - start + 1 <= chunk_size {
            prev = page;
            continue;
        }
        ranges.push((start, prev));
        start = page;
        prev = page;
    }
    ranges.push((start, prev));
    ranges
}

pub fn page_range_metadata(start: u32, end: u32) -> BTreeMap<String, String> {
    let mut meta = BTreeMap::new();
    meta.insert("ingest_route".to_string(), "visual".to_string());
    meta.insert("page_range_start".to_string(), start.to_string());
    meta.insert("page_range_end".to_string(), end.to_string());
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_page_ranges_groups_contiguous_pages_up_to_chunk_size() {
        assert_eq!(chunk_page_ranges(&[1, 2, 3, 4], 4), vec![(1, 4)]);
        assert_eq!(chunk_page_ranges(&[1, 2, 4, 5], 4), vec![(1, 2), (4, 5)]);
        assert_eq!(chunk_page_ranges(&[1, 3, 5], 4), vec![(1, 1), (3, 3), (5, 5)]);
    }

    #[test]
    fn chunk_page_ranges_dedups_and_sorts_input() {
        assert_eq!(chunk_page_ranges(&[4, 2, 3, 2], 4), vec![(2, 4)]);
    }

    #[test]
    fn pages_per_visual_chunk_stays_within_supported_bounds() {
        let value = pages_per_visual_chunk();
        assert!((1..=5).contains(&value));
    }
}
