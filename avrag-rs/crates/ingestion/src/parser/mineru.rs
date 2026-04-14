use std::collections::BTreeMap;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{Duration, sleep};
use tracing::{debug, info};

use super::{NormalizedDocument, ParsedUnit};

#[derive(Debug, Clone)]
pub struct MineruConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout_ms: u64,
}

impl MineruConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("MINERU_BASE_URL").ok()?;
        let api_key = std::env::var("MINERU_API_KEY").ok()?;
        let timeout_ms = std::env::var("MINERU_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30000);

        if base_url.is_empty() || api_key.is_empty() {
            return None;
        }

        Some(Self {
            base_url,
            api_key,
            timeout_ms,
        })
    }
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct TaskStatus {
    status: String,
    markdown_url: Option<String>,
    images: Option<Vec<ImageInfo>>,
}

#[derive(Debug, Deserialize, Clone)]
struct ImageInfo {
    filename: String,
    url: String,
    page: u32,
    caption: Option<String>,
}

pub struct MineruClient {
    config: MineruConfig,
    client: Client,
}

impl MineruClient {
    pub fn new(config: MineruConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    pub async fn parse(&self, bytes: &[u8], filename: &str) -> Result<NormalizedDocument> {
        info!(filename, "Starting MinerU precise parse");

        let task_id = self.upload_file(bytes, filename).await?;
        debug!(task_id, "File uploaded, waiting for processing");

        self.wait_for_completion(&task_id).await?;
        let result = self.fetch_result(&task_id, filename).await?;

        info!(filename, "MinerU parse completed");
        Ok(result)
    }

    async fn upload_file(&self, bytes: &[u8], filename: &str) -> Result<String> {
        let url = format!("{}/v1/parse/upload", self.config.base_url);

        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(bytes.to_vec()).file_name(filename.to_string()),
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await
            .context("Failed to upload file to MinerU")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU upload failed: {} - {}", status, body);
        }

        let upload_response: UploadResponse = response
            .json()
            .await
            .context("Failed to parse MinerU upload response")?;

        Ok(upload_response.task_id)
    }

    async fn wait_for_completion(&self, task_id: &str) -> Result<()> {
        let url = format!("{}/v1/parse/status/{}", self.config.base_url, task_id);
        let mut attempts = 0;
        let max_attempts = 60;

        loop {
            if attempts >= max_attempts {
                anyhow::bail!("MinerU task timed out after {} attempts", max_attempts);
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await
                .context("Failed to check MinerU task status")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("MinerU status check failed: {} - {}", status, body);
            }

            let status: TaskStatus = response
                .json()
                .await
                .context("Failed to parse MinerU status response")?;

            match status.status.as_str() {
                "completed" => return Ok(()),
                "failed" => anyhow::bail!("MinerU task failed"),
                "processing" => {
                    debug!(task_id, attempt = attempts, "MinerU task still processing");
                    sleep(Duration::from_secs(2)).await;
                }
                _ => {
                    debug!(task_id, status = %status.status, "MinerU task status");
                    sleep(Duration::from_secs(2)).await;
                }
            }

            attempts += 1;
        }
    }

    async fn fetch_result(&self, task_id: &str, filename: &str) -> Result<NormalizedDocument> {
        let url = format!("{}/v1/parse/result/{}", self.config.base_url, task_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to fetch MinerU result")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU result fetch failed: {} - {}", status, body);
        }

        let status: TaskStatus = response
            .json()
            .await
            .context("Failed to parse MinerU result")?;

        let markdown_url = status
            .markdown_url
            .context("MinerU result missing markdown URL")?;

        let markdown = self.fetch_markdown(&markdown_url).await?;
        let images = status.images.unwrap_or_default();

        self.normalize_result(filename, markdown, images)
    }

    async fn fetch_markdown(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to fetch markdown from MinerU")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch markdown: {}", response.status());
        }

        Ok(response
            .text()
            .await
            .context("Failed to read markdown content")?)
    }

    fn normalize_result(
        &self,
        filename: &str,
        markdown: String,
        images: Vec<ImageInfo>,
    ) -> Result<NormalizedDocument> {
        let blocks = markdown_blocks(&markdown);
        let title = blocks
            .iter()
            .find_map(|block| block.strip_prefix("# ").map(str::trim))
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| filename.to_string());

        let mut units = Vec::new();
        for block in blocks
            .iter()
            .filter(|block| !looks_like_image_reference(block))
        {
            units.push(ParsedUnit::new_text(
                1,
                block.clone(),
                "mineru_precise".to_string(),
            ));
        }

        for image in images {
            let context = image_context(&blocks, &image);
            let normalized_text = [
                image.caption.clone().unwrap_or_default(),
                context.clone().unwrap_or_default(),
                format!("[Image: {}]", image.filename),
            ]
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

            units.push(ParsedUnit::new_image_with_context(
                image.page.max(1),
                normalized_text,
                image.url,
                image.caption,
                context,
                "mineru_precise".to_string(),
            ));
        }

        Ok(NormalizedDocument {
            title,
            units,
            metadata: BTreeMap::new(),
        })
    }
}

fn markdown_blocks(markdown: &str) -> Vec<String> {
    markdown
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn looks_like_image_reference(block: &str) -> bool {
    let lower = block.to_lowercase();
    lower.starts_with("![")
        || lower.contains("<img")
        || lower.contains(".png")
        || lower.contains(".jpg")
}

fn image_context(blocks: &[String], image: &ImageInfo) -> Option<String> {
    let caption = image.caption.as_deref().map(str::trim).unwrap_or("");
    let filename = image.filename.as_str();

    if let Some(index) = blocks.iter().position(|block| {
        block.contains(filename) || (!caption.is_empty() && block.contains(caption))
    }) {
        let start = index.saturating_sub(1);
        let end = (index + 2).min(blocks.len());
        let joined = blocks[start..end]
            .iter()
            .filter(|block| !looks_like_image_reference(block))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
        return (!joined.trim().is_empty()).then_some(joined);
    }

    if !caption.is_empty() {
        return Some(caption.to_string());
    }

    blocks
        .iter()
        .find(|block| !looks_like_image_reference(block))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mineru_config_from_env() {
        unsafe {
            std::env::set_var("MINERU_BASE_URL", "https://api.mineru.com");
            std::env::set_var("MINERU_API_KEY", "test-key");
        }

        let config = MineruConfig::from_env().unwrap();
        assert_eq!(config.base_url, "https://api.mineru.com");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.timeout_ms, 30000);

        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
        }
    }

    #[test]
    fn test_mineru_config_missing_env() {
        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
        }

        let config = MineruConfig::from_env();
        assert!(config.is_none());
    }

    #[test]
    fn test_markdown_blocks_split_paragraphs() {
        let blocks = markdown_blocks("# Title\n\nParagraph one\n\nParagraph two");
        assert_eq!(blocks.len(), 3);
    }
}
