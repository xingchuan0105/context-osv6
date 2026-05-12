use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use lopdf::Document;
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
struct ExtractV4UploadUrlData {
    batch_id: String,
    file_urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractV4BatchData {
    extract_result: Vec<ExtractV4TaskData>,
}

#[derive(Debug, Deserialize)]
struct ExtractV4TaskData {
    file_name: Option<String>,
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

    async fn parse_with_page_filter_v4_remote(
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

    async fn parse_with_page_filter_v4_upload(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: Option<&[u32]>,
        is_ocr: bool,
    ) -> Result<NormalizedDocument> {
        let upload = prepare_v4_file_upload_payload(bytes, filename, page_numbers)?;
        info!(
            filename,
            is_ocr,
            upload_bytes = upload.bytes.len(),
            uses_remote_page_ranges = upload.page_numbers.is_some(),
            "MinerU v4 local file upload prepared"
        );
        let (batch_id, upload_url) = self
            .create_file_upload_batch_v4(filename, upload.page_numbers, is_ocr)
            .await?;
        info!(batch_id, filename, "MinerU v4 file upload URL created");
        self.upload_file_to_signed_url_v4(&upload_url, &upload.bytes)
            .await?;
        info!(
            batch_id,
            filename, "MinerU v4 file uploaded, waiting for batch result"
        );

        let task_result = self
            .wait_for_batch_completion_v4(&batch_id, filename)
            .await?;
        let zip_url = task_result
            .full_zip_url
            .filter(|value| !value.trim().is_empty())
            .context("MinerU v4 batch task completed without full_zip_url")?;
        let (markdown, images) = self.fetch_zip_payload_v4(&zip_url, &batch_id).await?;
        self.normalize_result(filename, markdown, images)
    }

    async fn parse_pdf_pages_v4_upload_batch(
        &self,
        bytes: &[u8],
        filename: &str,
        page_numbers: &[u32],
    ) -> Result<NormalizedDocument> {
        let mut files = Vec::new();
        let mut skipped_before_upload = Vec::new();
        for page_number in page_numbers {
            match prepare_v4_ocr_page_upload(bytes, filename, *page_number).with_context(|| {
                format!("Failed to prepare MinerU OCR upload for page {page_number}")
            })? {
                Some(file) => files.push(file),
                None => skipped_before_upload.push(*page_number),
            }
        }

        if files.is_empty() {
            info!(
                filename,
                pages = page_numbers.len(),
                skipped_before_upload = skipped_before_upload.len(),
                "Skipping MinerU v4 OCR batch because all pages are blank or low-value"
            );
            return Ok(NormalizedDocument {
                title: filename.to_string(),
                units: page_numbers
                    .iter()
                    .map(|page_number| skipped_ocr_page_unit(*page_number))
                    .collect(),
                metadata: BTreeMap::new(),
            });
        }

        let filenames = files
            .iter()
            .map(|file| file.filename.clone())
            .collect::<Vec<_>>();
        let (batch_id, upload_urls) = self
            .create_file_upload_batch_v4_files(&filenames, true)
            .await?;
        if upload_urls.len() != files.len() {
            anyhow::bail!(
                "MinerU v4 file upload batch returned {} upload URLs for {} files",
                upload_urls.len(),
                files.len()
            );
        }
        info!(
            batch_id,
            filename,
            file_count = files.len(),
            skipped_before_upload = skipped_before_upload.len(),
            "MinerU v4 OCR batch upload URLs created"
        );

        for (file, upload_url) in files.iter().zip(upload_urls.iter()) {
            self.upload_file_to_signed_url_v4(upload_url, &file.bytes)
                .await
                .with_context(|| {
                    format!("Failed to upload MinerU OCR page {}", file.page_number)
                })?;
        }
        info!(
            batch_id,
            filename,
            file_count = files.len(),
            "MinerU v4 OCR batch files uploaded, waiting for batch result"
        );

        let results = self
            .wait_for_batch_completion_v4_all(&batch_id, files.len())
            .await?;
        let mut results_by_file = results
            .into_iter()
            .filter_map(|result| {
                result
                    .file_name
                    .clone()
                    .map(|file_name| (file_name, result))
            })
            .collect::<BTreeMap<_, _>>();

        let mut units = skipped_before_upload
            .iter()
            .map(|page_number| skipped_ocr_page_unit(*page_number))
            .collect::<Vec<_>>();
        let mut title: Option<String> = None;
        let mut skipped_after_ocr = 0usize;
        for file in files {
            let task_result = results_by_file.remove(&file.filename).with_context(|| {
                format!("MinerU v4 batch result missing file {}", file.filename)
            })?;
            let zip_url = task_result
                .full_zip_url
                .filter(|value| !value.trim().is_empty())
                .with_context(|| {
                    format!(
                        "MinerU v4 batch task for page {} completed without full_zip_url",
                        file.page_number
                    )
                })?;
            let (markdown, images) = self.fetch_zip_payload_v4(&zip_url, &batch_id).await?;
            let mut page_document = self.normalize_result(&file.filename, markdown, images)?;
            title = title.take().or_else(|| Some(page_document.title.clone()));
            for unit in &mut page_document.units {
                unit.page = file.page_number;
                unit.parser_backend = "mineru_pdf_ocr".to_string();
            }
            if is_low_value_ocr_document(&page_document) {
                skipped_after_ocr += 1;
                info!(
                    filename,
                    page_number = file.page_number,
                    "Skipping low-value MinerU OCR result"
                );
                units.push(skipped_ocr_page_unit(file.page_number));
                continue;
            }
            units.extend(page_document.units);
        }

        info!(
            filename,
            pages = page_numbers.len(),
            uploaded = filenames.len(),
            skipped_before_upload = skipped_before_upload.len(),
            skipped_after_ocr,
            units = units.len(),
            "MinerU v4 OCR batch completed"
        );

        Ok(NormalizedDocument {
            title: title.unwrap_or_else(|| filename.to_string()),
            units,
            metadata: BTreeMap::new(),
        })
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

    async fn create_file_upload_batch_v4(
        &self,
        filename: &str,
        page_numbers: Option<&[u32]>,
        is_ocr: bool,
    ) -> Result<(String, String)> {
        let url = format!("{}/file-urls/batch", self.config.base_url);
        let payload = build_file_upload_batch_payload_v4(filename, page_numbers, is_ocr);
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .context("Failed to create MinerU v4 file upload batch")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU v4 file upload batch failed: {} - {}", status, body);
        }

        let body: ExtractV4Envelope<ExtractV4UploadUrlData> = response
            .json()
            .await
            .context("Failed to decode MinerU v4 file upload batch response")?;
        if body.code != 0 {
            anyhow::bail!(
                "MinerU v4 file upload batch failed: code={} msg={}",
                body.code,
                body.msg
            );
        }

        let data = body
            .data
            .context("MinerU v4 file upload batch response missing data payload")?;
        let upload_url = data
            .file_urls
            .into_iter()
            .next()
            .context("MinerU v4 file upload batch response missing upload URL")?;
        Ok((data.batch_id, upload_url))
    }

    async fn create_file_upload_batch_v4_files(
        &self,
        filenames: &[String],
        is_ocr: bool,
    ) -> Result<(String, Vec<String>)> {
        let url = format!("{}/file-urls/batch", self.config.base_url);
        let payload = build_file_upload_batch_payload_v4_files(filenames, is_ocr);
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .context("Failed to create MinerU v4 file upload batch")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU v4 file upload batch failed: {} - {}", status, body);
        }

        let body: ExtractV4Envelope<ExtractV4UploadUrlData> = response
            .json()
            .await
            .context("Failed to decode MinerU v4 file upload batch response")?;
        if body.code != 0 {
            anyhow::bail!(
                "MinerU v4 file upload batch failed: code={} msg={}",
                body.code,
                body.msg
            );
        }

        let data = body
            .data
            .context("MinerU v4 file upload batch response missing data payload")?;
        Ok((data.batch_id, data.file_urls))
    }

    async fn upload_file_to_signed_url_v4(&self, upload_url: &str, bytes: &[u8]) -> Result<()> {
        let response = self
            .client
            .put(upload_url)
            .body(bytes.to_vec())
            .send()
            .await
            .context("Failed to upload file to MinerU v4 signed URL")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MinerU v4 signed URL upload failed: {} - {}", status, body);
        }
        Ok(())
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

    async fn wait_for_batch_completion_v4(
        &self,
        batch_id: &str,
        filename: &str,
    ) -> Result<ExtractV4TaskData> {
        let url = format!(
            "{}/extract-results/batch/{}",
            self.config.base_url, batch_id
        );
        let mut attempts = 0;

        loop {
            if attempts >= DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS {
                anyhow::bail!(
                    "MinerU v4 batch task timed out after {} attempts",
                    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS
                );
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await
                .with_context(|| format!("Failed to query MinerU v4 batch {batch_id}"))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("MinerU v4 batch query failed: {} - {}", status, body);
            }

            let body: ExtractV4Envelope<ExtractV4BatchData> = response
                .json()
                .await
                .context("Failed to decode MinerU v4 batch query response")?;
            if body.code != 0 {
                anyhow::bail!(
                    "MinerU v4 batch query failed: code={} msg={}",
                    body.code,
                    body.msg
                );
            }

            let data = body
                .data
                .context("MinerU v4 batch query missing data payload")?;
            let mut results = data.extract_result;
            if results.is_empty() {
                debug!(
                    batch_id,
                    attempt = attempts,
                    "MinerU v4 batch result is empty"
                );
                sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                attempts += 1;
                continue;
            }
            let index = results
                .iter()
                .position(|result| result.file_name.as_deref() == Some(filename))
                .unwrap_or(0);
            let result = results.remove(index);

            match result.state.as_str() {
                "done" => return Ok(result),
                "failed" => {
                    anyhow::bail!(
                        "MinerU v4 batch task failed: {}",
                        result
                            .err_msg
                            .unwrap_or_else(|| "unknown error".to_string())
                    );
                }
                "waiting-file" | "pending" | "running" | "converting" => {
                    debug!(
                        batch_id,
                        attempt = attempts,
                        state = %result.state,
                        "MinerU v4 batch task still running"
                    );
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
                other => {
                    debug!(
                        batch_id,
                        attempt = attempts,
                        state = other,
                        "Unexpected MinerU v4 batch state"
                    );
                    sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                }
            }

            attempts += 1;
        }
    }

    async fn wait_for_batch_completion_v4_all(
        &self,
        batch_id: &str,
        expected_count: usize,
    ) -> Result<Vec<ExtractV4TaskData>> {
        let url = format!(
            "{}/extract-results/batch/{}",
            self.config.base_url, batch_id
        );
        let mut attempts = 0;

        loop {
            if attempts >= DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS {
                anyhow::bail!(
                    "MinerU v4 batch task timed out after {} attempts",
                    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS
                );
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await
                .with_context(|| format!("Failed to query MinerU v4 batch {batch_id}"))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("MinerU v4 batch query failed: {} - {}", status, body);
            }

            let body: ExtractV4Envelope<ExtractV4BatchData> = response
                .json()
                .await
                .context("Failed to decode MinerU v4 batch query response")?;
            if body.code != 0 {
                anyhow::bail!(
                    "MinerU v4 batch query failed: code={} msg={}",
                    body.code,
                    body.msg
                );
            }

            let data = body
                .data
                .context("MinerU v4 batch query missing data payload")?;
            let results = data.extract_result;
            if results.len() < expected_count {
                debug!(
                    batch_id,
                    attempt = attempts,
                    expected_count,
                    result_count = results.len(),
                    "MinerU v4 batch result is incomplete"
                );
                sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
                attempts += 1;
                continue;
            }
            if let Some(failed) = results.iter().find(|result| result.state == "failed") {
                anyhow::bail!(
                    "MinerU v4 batch task failed: {}",
                    failed
                        .err_msg
                        .clone()
                        .unwrap_or_else(|| "unknown error".to_string())
                );
            }
            if results.iter().all(|result| result.state == "done") {
                return Ok(results);
            }

            debug!(
                batch_id,
                attempt = attempts,
                expected_count,
                result_count = results.len(),
                "MinerU v4 batch tasks still running"
            );
            sleep(Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS)).await;
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

        response
            .text()
            .await
            .context("Failed to read markdown content")
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

struct MineruV4FileUploadPayload<'a> {
    bytes: Vec<u8>,
    page_numbers: Option<&'a [u32]>,
}

struct MineruV4PageUploadFile {
    filename: String,
    bytes: Vec<u8>,
    page_number: u32,
}

fn prepare_v4_file_upload_payload<'a>(
    bytes: &[u8],
    filename: &str,
    page_numbers: Option<&'a [u32]>,
) -> Result<MineruV4FileUploadPayload<'a>> {
    if let Some(pages) = page_numbers
        && filename.to_ascii_lowercase().ends_with(".pdf") {
            return Ok(MineruV4FileUploadPayload {
                bytes: extract_pdf_pages(bytes, pages)?,
                page_numbers: None,
            });
        }

    Ok(MineruV4FileUploadPayload {
        bytes: bytes.to_vec(),
        page_numbers,
    })
}

fn prepare_v4_ocr_page_upload(
    bytes: &[u8],
    filename: &str,
    page_number: u32,
) -> Result<Option<MineruV4PageUploadFile>> {
    let page_bytes = extract_pdf_pages(bytes, &[page_number])?;
    if is_low_value_pdf_upload_page(&page_bytes)? {
        return Ok(None);
    }

    Ok(Some(MineruV4PageUploadFile {
        filename: v4_page_upload_filename(filename, page_number),
        bytes: page_bytes,
        page_number,
    }))
}

fn v4_page_upload_filename(filename: &str, page_number: u32) -> String {
    let path = PathBuf::from(filename);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "document".to_string());
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "pdf".to_string());
    format!("{stem}-page-{page_number:04}.{extension}")
}

fn is_low_value_pdf_upload_page(bytes: &[u8]) -> Result<bool> {
    let document =
        Document::load_mem(bytes).context("Failed to load PDF page for OCR skip check")?;
    let Some(page_id) = document.get_pages().values().next().copied() else {
        return Ok(true);
    };
    let content = document.get_page_content(page_id).unwrap_or_default();
    Ok(!pdf_content_has_renderable_operations(&content))
}

fn pdf_content_has_renderable_operations(content: &[u8]) -> bool {
    let content = String::from_utf8_lossy(content);
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.contains(" Tj")
        || compact.contains(" TJ")
        || compact.contains(" '")
        || compact.contains(" \"")
        || compact.contains(" Do")
        || compact.contains(" BI")
}

fn extract_pdf_pages(bytes: &[u8], page_numbers: &[u32]) -> Result<Vec<u8>> {
    if page_numbers.is_empty() {
        anyhow::bail!("MinerU v4 PDF upload page filter must not be empty");
    }
    if let [page_number] = page_numbers
        && let Ok(split) = extract_single_pdf_page_with_pdfseparate(bytes, *page_number) {
            return Ok(split);
        }

    let mut document =
        Document::load_mem(bytes).context("Failed to load PDF for MinerU page upload")?;
    let selected = page_numbers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let pages = document.get_pages();
    for page_number in &selected {
        if !pages.contains_key(page_number) {
            anyhow::bail!("PDF page {} is not present for MinerU upload", page_number);
        }
    }

    let pages_to_delete = pages
        .keys()
        .copied()
        .filter(|page_number| !selected.contains(page_number))
        .collect::<Vec<_>>();
    document.delete_pages(&pages_to_delete);
    document.prune_objects();
    document.renumber_objects();

    let mut output = Vec::new();
    document
        .save_to(&mut output)
        .context("Failed to serialize MinerU page-filtered PDF upload")?;
    Ok(output)
}

fn extract_single_pdf_page_with_pdfseparate(bytes: &[u8], page_number: u32) -> Result<Vec<u8>> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_dir = std::env::temp_dir().join(format!(
        "avrag-mineru-pdf-split-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&temp_dir).with_context(|| {
        format!(
            "Failed to create MinerU PDF split temp dir {}",
            temp_dir.display()
        )
    })?;

    let result = (|| -> Result<Vec<u8>> {
        let input_path = temp_dir.join("input.pdf");
        fs::write(&input_path, bytes).with_context(|| {
            format!(
                "Failed to write MinerU PDF split input {}",
                input_path.display()
            )
        })?;
        let output_pattern = temp_dir.join("page-%d.pdf");
        let output = Command::new("pdfseparate")
            .arg("-f")
            .arg(page_number.to_string())
            .arg("-l")
            .arg(page_number.to_string())
            .arg(&input_path)
            .arg(&output_pattern)
            .output()
            .context("Failed to run pdfseparate for MinerU PDF page upload")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pdfseparate failed for page {page_number}: {stderr}");
        }

        let output_path = temp_dir.join(format!("page-{page_number}.pdf"));
        fs::read(&output_path).with_context(|| {
            format!(
                "Failed to read MinerU PDF split output {}",
                output_path.display()
            )
        })
    })();
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn should_use_remote_extract_v4(source_url: Option<&str>) -> bool {
    source_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(is_http_source_url)
}

fn is_http_source_url(source_url: &str) -> bool {
    source_url.starts_with("http://") || source_url.starts_with("https://")
}

fn require_remote_source_url(source_url: Option<&str>, filename: &str) -> Result<String> {
    let source_url = source_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| {
            format!("MinerU v4 parse for {filename} requires a source URL, but none was provided")
        })?;
    if !is_http_source_url(source_url) {
        anyhow::bail!(
            "MinerU v4 requires an HTTP(S) source URL; got {} for {}",
            source_url,
            filename
        );
    }
    Ok(source_url.to_string())
}

fn build_file_upload_batch_payload_v4(
    filename: &str,
    page_numbers: Option<&[u32]>,
    is_ocr: bool,
) -> serde_json::Value {
    let mut file = serde_json::json!({ "name": filename });
    if let Some(page_numbers) = page_numbers {
        file["page_ranges"] = serde_json::json!(format_page_ranges(page_numbers));
    }
    if is_ocr {
        file["is_ocr"] = serde_json::json!(true);
    }

    serde_json::json!({
        "files": [file],
        "model_version": "vlm",
    })
}

fn build_file_upload_batch_payload_v4_files(
    filenames: &[String],
    is_ocr: bool,
) -> serde_json::Value {
    let files = filenames
        .iter()
        .map(|filename| {
            let mut file = serde_json::json!({ "name": filename });
            if is_ocr {
                file["is_ocr"] = serde_json::json!(true);
            }
            file
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "files": files,
        "model_version": "vlm",
    })
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
            if markdown.is_none() || !content.trim().is_empty() {
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

fn skipped_ocr_page_unit(page_number: u32) -> ParsedUnit {
    let mut unit = ParsedUnit::new_text(page_number, String::new(), "mineru_pdf_ocr".to_string());
    unit.metadata
        .insert("ocr_skipped".to_string(), "low_value".to_string());
    unit
}

fn is_low_value_ocr_document(document: &NormalizedDocument) -> bool {
    let signal_chars = document
        .units
        .iter()
        .map(|unit| meaningful_ocr_signal_chars(&unit.text))
        .sum::<usize>();
    signal_chars < 8
}

fn meaningful_ocr_signal_chars(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("[Image:"))
        .flat_map(str::chars)
        .filter(|ch| ch.is_alphanumeric())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Dictionary, Object, Stream, dictionary};
    use std::io::Write;
    use zip::write::SimpleFileOptions;

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
    fn mineru_v4_zip_accepts_empty_markdown_output() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("full.md", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let (markdown, images) = extract_markdown_and_images_from_zip(&bytes, "empty-md").unwrap();

        assert_eq!(markdown, "");
        assert!(images.is_empty());
    }

    #[test]
    fn page_ranges_compact_contiguous_pages() {
        assert_eq!(format_page_ranges(&[1, 2, 3, 5, 8, 9]), "1-3,5,8-9");
    }

    #[test]
    fn v4_source_selection_treats_file_url_as_local_upload() {
        assert!(should_use_remote_extract_v4(Some(
            "https://example.com/demo.pdf"
        )));
        assert!(should_use_remote_extract_v4(Some(
            "http://example.com/demo.pdf"
        )));
        assert!(!should_use_remote_extract_v4(Some(
            "file://tenant/notebook/doc/demo.pdf"
        )));
        assert!(!should_use_remote_extract_v4(None));
    }

    #[test]
    fn v4_file_upload_batch_payload_includes_page_filter_and_ocr_flag() {
        let payload = build_file_upload_batch_payload_v4("demo.pdf", Some(&[3, 1, 2]), true);

        assert_eq!(payload["model_version"], "vlm");
        assert_eq!(payload["files"][0]["name"], "demo.pdf");
        assert_eq!(payload["files"][0]["page_ranges"], "1-3");
        assert_eq!(payload["files"][0]["is_ocr"], true);
    }

    #[test]
    fn v4_file_upload_batch_payload_keeps_ocr_flag_without_page_filter() {
        let payload = build_file_upload_batch_payload_v4("single-page.pdf", None, true);

        assert_eq!(payload["files"][0]["name"], "single-page.pdf");
        assert_eq!(payload["files"][0]["is_ocr"], true);
        assert!(payload["files"][0].get("page_ranges").is_none());
    }

    #[test]
    fn v4_pdf_page_filter_upload_splits_pdf_and_omits_remote_page_ranges() {
        let pdf = two_page_pdf_fixture();

        let upload = prepare_v4_file_upload_payload(&pdf, "demo.pdf", Some(&[2])).unwrap();

        assert!(upload.page_numbers.is_none());
        let split = Document::load_mem(&upload.bytes).unwrap();
        assert_eq!(split.get_pages().len(), 1);
    }

    #[test]
    fn v4_file_upload_batch_payload_supports_multiple_ocr_files() {
        let filenames = vec![
            "demo-page-0001.pdf".to_string(),
            "demo-page-0002.pdf".to_string(),
        ];

        let payload = build_file_upload_batch_payload_v4_files(&filenames, true);

        assert_eq!(payload["model_version"], "vlm");
        assert_eq!(payload["files"].as_array().unwrap().len(), 2);
        assert_eq!(payload["files"][0]["name"], "demo-page-0001.pdf");
        assert_eq!(payload["files"][1]["name"], "demo-page-0002.pdf");
        assert_eq!(payload["files"][0]["is_ocr"], true);
        assert!(payload["files"][0].get("page_ranges").is_none());
    }

    #[test]
    fn v4_ocr_page_upload_skips_blank_pdf_page() {
        let pdf = one_page_pdf_fixture(b"BT ET");

        let upload = prepare_v4_ocr_page_upload(&pdf, "demo.pdf", 1).unwrap();

        assert!(upload.is_none());
    }

    #[test]
    fn v4_ocr_page_upload_keeps_renderable_pdf_page() {
        let pdf = one_page_pdf_fixture(b"q /Im0 Do Q");

        let upload = prepare_v4_ocr_page_upload(&pdf, "demo.pdf", 1)
            .unwrap()
            .unwrap();

        assert_eq!(upload.filename, "demo-page-0001.pdf");
        assert_eq!(upload.page_number, 1);
        assert!(!upload.bytes.is_empty());
    }

    #[test]
    fn skipped_ocr_page_unit_preserves_requested_page() {
        let unit = skipped_ocr_page_unit(42);

        assert_eq!(unit.page, 42);
        assert_eq!(unit.parser_backend, "mineru_pdf_ocr");
        assert_eq!(
            unit.metadata.get("ocr_skipped").map(String::as_str),
            Some("low_value")
        );
    }

    #[test]
    fn v4_low_value_ocr_detection_skips_empty_and_page_number_only_text() {
        let empty = NormalizedDocument {
            title: "empty".to_string(),
            units: Vec::new(),
            metadata: BTreeMap::new(),
        };
        assert!(is_low_value_ocr_document(&empty));

        let page_number_only = NormalizedDocument {
            title: "page".to_string(),
            units: vec![ParsedUnit::new_text(
                1,
                "342".to_string(),
                "mineru_pdf_ocr".to_string(),
            )],
            metadata: BTreeMap::new(),
        };
        assert!(is_low_value_ocr_document(&page_number_only));

        let useful = NormalizedDocument {
            title: "useful".to_string(),
            units: vec![ParsedUnit::new_text(
                1,
                "meaningful OCR text".to_string(),
                "mineru_pdf_ocr".to_string(),
            )],
            metadata: BTreeMap::new(),
        };
        assert!(!is_low_value_ocr_document(&useful));
    }

    fn one_page_pdf_fixture(content: &[u8]) -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
        let catalog_id = document.new_object_id();

        document.objects.insert(
            page_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content_id,
            }
            .into(),
        );
        document.objects.insert(
            pages_id,
            dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }
            .into(),
        );
        document.objects.insert(
            catalog_id,
            dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }
            .into(),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn two_page_pdf_fixture() -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page1_id = document.new_object_id();
        let page2_id = document.new_object_id();
        let content1_id = document.add_object(Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let content2_id = document.add_object(Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let catalog_id = document.new_object_id();

        document.objects.insert(
            page1_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content1_id,
            }
            .into(),
        );
        document.objects.insert(
            page2_id,
            dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
                "Contents" => content2_id,
            }
            .into(),
        );
        document.objects.insert(
            pages_id,
            dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page1_id), Object::Reference(page2_id)],
                "Count" => 2,
            }
            .into(),
        );
        document.objects.insert(
            catalog_id,
            dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }
            .into(),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }
}
