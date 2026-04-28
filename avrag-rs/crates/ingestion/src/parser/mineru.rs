use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
    path::PathBuf,
};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{Duration, sleep};
use tracing::{debug, info};
use zip::ZipArchive;

use super::{NormalizedDocument, ParsedUnit};

const DEFAULT_MINERU_BASE_URL: &str = "https://mineru.net/api/v4";
const DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS: usize = 90;
const DEFAULT_POLL_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MineruApiMode {
    LegacyV1Upload,
    ExtractV4,
}

#[derive(Debug, Clone)]
pub struct MineruConfig {
    pub base_url: String,
    pub api_key: String,
    pub timeout_ms: u64,
    api_mode: MineruApiMode,
}

impl MineruConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("MINERU_API_KEY").ok()?;
        if api_key.trim().is_empty() {
            return None;
        }

        let base_url = std::env::var("MINERU_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MINERU_BASE_URL.to_string());

        let api_mode = std::env::var("MINERU_API_MODE")
            .ok()
            .and_then(|value| parse_api_mode(&value))
            .unwrap_or_else(|| infer_api_mode_from_base_url(&base_url));

        let timeout_ms = std::env::var("MINERU_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30000);

        Some(Self {
            base_url,
            api_key,
            timeout_ms,
            api_mode,
        })
    }
}

#[derive(Debug, Deserialize)]
struct LegacyUploadResponse {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct LegacyTaskStatus {
    status: String,
    markdown_url: Option<String>,
    images: Option<Vec<ImageInfo>>,
}

#[derive(Debug, Deserialize)]
struct ExtractV4Envelope<T> {
    code: i64,
    msg: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct ExtractV4CreateTaskData {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct ExtractV4TaskData {
    state: String,
    err_msg: Option<String>,
    full_zip_url: Option<String>,
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
                let source_url = require_remote_source_url(source_url, filename)?;
                self.parse_with_page_filter_v4(filename, &source_url, None, false)
                    .await?
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
                let source_url = require_remote_source_url(source_url, filename)?;
                for page_number in page_numbers {
                    let mut page_document = self
                        .parse_with_page_filter_v4(
                            filename,
                            &source_url,
                            Some(&[*page_number]),
                            true,
                        )
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
        }

        Ok(NormalizedDocument {
            title: title.unwrap_or_else(|| filename.to_string()),
            units,
            metadata: BTreeMap::new(),
        })
    }

    async fn parse_with_page_filter_legacy(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: Option<&[u32]>,
    ) -> Result<NormalizedDocument> {
        let task_id = self
            .upload_file_legacy(bytes, filename, page_numbers)
            .await?;
        debug!(task_id, "File uploaded, waiting for processing");

        self.wait_for_completion_legacy(&task_id).await?;
        self.fetch_result_legacy(&task_id, filename).await
    }

    async fn parse_with_page_filter_v4(
        &self,
        filename: &str,
        source_url: &str,
        page_numbers: Option<&[u32]>,
        is_ocr: bool,
    ) -> Result<NormalizedDocument> {
        let task_id = self
            .create_extract_task_v4(source_url, page_numbers, is_ocr)
            .await?;
        debug!(task_id, source_url, "MinerU v4 task created");

        let task_result = self.wait_for_completion_v4(&task_id).await?;
        let zip_url = task_result
            .full_zip_url
            .filter(|value| !value.trim().is_empty())
            .context("MinerU v4 task completed without full_zip_url")?;
        let (markdown, images) = self.fetch_zip_payload_v4(&zip_url, &task_id).await?;
        self.normalize_result(filename, markdown, images)
    }

    async fn upload_file_legacy(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: Option<&[u32]>,
    ) -> Result<String> {
        let url = format!("{}/v1/parse/upload", self.config.base_url);

        let mut form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(bytes.to_vec()).file_name(filename.to_string()),
        );
        if let Some(page_numbers) = page_numbers {
            form = form.text(
                "page_numbers",
                serde_json::to_string(page_numbers)
                    .context("Failed to serialize MinerU page_numbers payload")?,
            );
        }

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

        let upload_response: LegacyUploadResponse = response
            .json()
            .await
            .context("Failed to parse MinerU upload response")?;

        Ok(upload_response.task_id)
    }

    async fn wait_for_completion_legacy(&self, task_id: &str) -> Result<()> {
        let url = format!("{}/v1/parse/status/{}", self.config.base_url, task_id);
        let mut attempts = 0;

        loop {
            if attempts >= DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS {
                anyhow::bail!(
                    "MinerU task timed out after {} attempts",
                    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS
                );
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

            let status: LegacyTaskStatus = response
                .json()
                .await
                .context("Failed to parse MinerU status response")?;

            match status.status.as_str() {
                "completed" => return Ok(()),
                "failed" => anyhow::bail!("MinerU task failed"),
                "processing" => {
                    debug!(task_id, attempt = attempts, "MinerU task still processing");
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
                _ => {
                    debug!(task_id, status = %status.status, "MinerU task status");
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
            }

            attempts += 1;
        }
    }

    async fn fetch_result_legacy(
        &self,
        task_id: &str,
        filename: &str,
    ) -> Result<NormalizedDocument> {
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

        let status: LegacyTaskStatus = response
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

    async fn create_extract_task_v4(
        &self,
        source_url: &str,
        page_numbers: Option<&[u32]>,
        is_ocr: bool,
    ) -> Result<String> {
        let url = format!("{}/extract/task", self.config.base_url);
        let mut payload = serde_json::json!({
            "url": source_url,
            "model_version": "vlm",
        });
        if let Some(page_numbers) = page_numbers {
            payload["page_ranges"] = serde_json::json!(format_page_ranges(page_numbers));
            payload["is_ocr"] = serde_json::json!(is_ocr);
        }

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .context("Failed to create MinerU v4 extract task")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU v4 create task failed: {} - {}", status, body);
        }

        let body: ExtractV4Envelope<ExtractV4CreateTaskData> = response
            .json()
            .await
            .context("Failed to decode MinerU v4 create task response")?;
        if body.code != 0 {
            anyhow::bail!(
                "MinerU v4 create task failed: code={} msg={}",
                body.code,
                body.msg
            );
        }

        body.data
            .map(|data| data.task_id)
            .context("MinerU v4 response missing task_id")
    }

    async fn wait_for_completion_v4(&self, task_id: &str) -> Result<ExtractV4TaskData> {
        let url = format!("{}/extract/task/{}", self.config.base_url, task_id);
        let mut attempts = 0;

        loop {
            if attempts >= DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS {
                anyhow::bail!(
                    "MinerU v4 task timed out after {} attempts",
                    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS
                );
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await
                .with_context(|| format!("Failed to query MinerU v4 task {}", task_id))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("MinerU v4 task query failed: {} - {}", status, body);
            }

            let body: ExtractV4Envelope<ExtractV4TaskData> = response
                .json()
                .await
                .context("Failed to decode MinerU v4 task query response")?;
            if body.code != 0 {
                anyhow::bail!(
                    "MinerU v4 task query failed: code={} msg={}",
                    body.code,
                    body.msg
                );
            }

            let data = body
                .data
                .context("MinerU v4 task query missing data payload")?;

            match data.state.as_str() {
                "done" => return Ok(data),
                "failed" => {
                    anyhow::bail!(
                        "MinerU v4 task failed: {}",
                        data.err_msg.unwrap_or_else(|| "unknown error".to_string())
                    );
                }
                "pending" | "running" | "converting" => {
                    debug!(
                        task_id,
                        attempt = attempts,
                        state = %data.state,
                        "MinerU v4 task still running"
                    );
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
                other => {
                    debug!(
                        task_id,
                        attempt = attempts,
                        state = other,
                        "Unexpected MinerU v4 state"
                    );
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
            }

            attempts += 1;
        }
    }

    async fn fetch_zip_payload_v4(
        &self,
        zip_url: &str,
        task_id: &str,
    ) -> Result<(String, Vec<ImageInfo>)> {
        let response = self
            .client
            .get(zip_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch MinerU v4 zip payload from {zip_url}"))?;
        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to fetch MinerU v4 zip payload: {}",
                response.status()
            );
        }
        let bytes = response
            .bytes()
            .await
            .context("Failed to read MinerU v4 zip payload")?;
        extract_markdown_and_images_from_zip(&bytes, task_id)
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

fn infer_api_mode_from_base_url(base_url: &str) -> MineruApiMode {
    if base_url.trim().contains("/api/v4") {
        MineruApiMode::ExtractV4
    } else {
        MineruApiMode::LegacyV1Upload
    }
}

fn parse_api_mode(raw: &str) -> Option<MineruApiMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "legacy_v1_upload" | "legacy" | "v1" => Some(MineruApiMode::LegacyV1Upload),
        "extract_v4" | "v4" => Some(MineruApiMode::ExtractV4),
        _ => None,
    }
}

fn require_remote_source_url(source_url: Option<&str>, filename: &str) -> Result<String> {
    let source_url = source_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| {
            format!("MinerU v4 parse for {filename} requires a source URL, but none was provided")
        })?;
    if !source_url.starts_with("http://") && !source_url.starts_with("https://") {
        anyhow::bail!(
            "MinerU v4 requires an HTTP(S) source URL; got {} for {}",
            source_url,
            filename
        );
    }
    Ok(source_url.to_string())
}

fn format_page_ranges(page_numbers: &[u32]) -> String {
    if page_numbers.is_empty() {
        return String::new();
    }

    let mut numbers = page_numbers.to_vec();
    numbers.sort_unstable();
    numbers.dedup();

    let mut ranges = Vec::new();
    let mut range_start = numbers[0];
    let mut previous = numbers[0];

    for current in numbers.into_iter().skip(1) {
        if current == previous + 1 {
            previous = current;
            continue;
        }
        if range_start == previous {
            ranges.push(range_start.to_string());
        } else {
            ranges.push(format!("{range_start}-{previous}"));
        }
        range_start = current;
        previous = current;
    }

    if range_start == previous {
        ranges.push(range_start.to_string());
    } else {
        ranges.push(format!("{range_start}-{previous}"));
    }

    ranges.join(",")
}

fn extract_markdown_and_images_from_zip(
    bytes: &[u8],
    task_id: &str,
) -> Result<(String, Vec<ImageInfo>)> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to open MinerU v4 zip payload archive")?;

    let mut markdown: Option<String> = None;
    let mut images = Vec::new();
    let image_dir = std::env::temp_dir()
        .join("mineru-v4")
        .join(format!("task-{task_id}"));
    std::fs::create_dir_all(&image_dir).with_context(|| {
        format!(
            "Failed to create MinerU temporary image directory {}",
            image_dir.display()
        )
    })?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("Failed to read zip entry #{index}"))?;
        if file.is_dir() {
            continue;
        }

        let entry_name = file.name().to_string();
        let lower = entry_name.to_ascii_lowercase();
        if lower.ends_with(".md") {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .context("Failed to read markdown from MinerU v4 zip")?;
            if !content.trim().is_empty() {
                markdown = Some(content);
            }
            continue;
        }

        if !is_supported_image_file(&lower) {
            continue;
        }

        let file_name = PathBuf::from(&entry_name)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("image-{index}.bin"));
        let file_path = image_dir.join(&file_name);

        let mut image_bytes = Vec::new();
        file.read_to_end(&mut image_bytes)
            .context("Failed to read image bytes from MinerU v4 zip")?;
        std::fs::write(&file_path, &image_bytes).with_context(|| {
            format!(
                "Failed to write temporary MinerU image {}",
                file_path.display()
            )
        })?;

        images.push(ImageInfo {
            filename: file_name.clone(),
            url: format!("temporary://{}", file_path.to_string_lossy()),
            page: infer_page_number_from_name(&entry_name).unwrap_or(1),
            caption: None,
        });
    }

    let markdown = markdown.context("MinerU v4 zip payload does not contain markdown output")?;
    Ok((markdown, images))
}

fn is_supported_image_file(path: &str) -> bool {
    path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".webp")
        || path.ends_with(".gif")
        || path.ends_with(".bmp")
}

fn infer_page_number_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let marker = lower.find("page")?;
    let suffix = &lower[marker + 4..];
    let digits = suffix
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u32>().ok()
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
    fn mineru_config_from_env() {
        unsafe {
            std::env::set_var("MINERU_BASE_URL", "https://mineru.net/api/v4");
            std::env::set_var("MINERU_API_KEY", "test-key");
            std::env::set_var("MINERU_API_MODE", "extract_v4");
        }

        let config = MineruConfig::from_env().unwrap();
        assert_eq!(config.base_url, "https://mineru.net/api/v4");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.api_mode, MineruApiMode::ExtractV4);

        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }
    }

    #[test]
    fn mineru_config_defaults_to_v4_base_url() {
        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::set_var("MINERU_API_KEY", "test-key");
            std::env::remove_var("MINERU_API_MODE");
        }

        let config = MineruConfig::from_env().unwrap();
        assert_eq!(config.base_url, DEFAULT_MINERU_BASE_URL);
        assert_eq!(config.api_mode, MineruApiMode::ExtractV4);

        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }
    }

    #[test]
    fn mineru_config_missing_env() {
        unsafe {
            std::env::remove_var("MINERU_BASE_URL");
            std::env::remove_var("MINERU_API_KEY");
            std::env::remove_var("MINERU_API_MODE");
        }

        let config = MineruConfig::from_env();
        assert!(config.is_none());
    }

    #[test]
    fn markdown_blocks_split_paragraphs() {
        let blocks = markdown_blocks("# Title\n\nParagraph one\n\nParagraph two");
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn page_ranges_compact_contiguous_pages() {
        assert_eq!(format_page_ranges(&[1, 2, 3, 5, 8, 9]), "1-3,5,8-9");
    }
}
