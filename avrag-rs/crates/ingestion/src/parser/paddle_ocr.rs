use anyhow::Context;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::IngestionError;

/// Default max single-file (PDF) size accepted for a Paddle OCR job.
///
/// Matches the provider's documented single-file limit (≤200MB & ≤1000 pages).
/// Rejects oversized inputs locally before any upload instead of letting a
/// rejected/hung job burn the retry budget. Configurable via
/// `PADDLE_OCR_MAX_FILE_SIZE_BYTES`.
const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 200 * 1024 * 1024;

/// Default max single-image size accepted for a Paddle OCR job.
///
/// Provider's documented single-image limit is ≤10MB (tighter than the file
/// limit). Configurable via `PADDLE_OCR_MAX_IMAGE_SIZE_BYTES`.
const DEFAULT_MAX_IMAGE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Bounded number of consecutive non-200 poll responses before failing, so a
/// provider that starts rejecting poll requests surfaces an error instead of
/// silently polling until the task timeout.
const MAX_CONSECUTIVE_POLL_FAILURES: u32 = 5;

#[derive(Debug, Clone)]
pub struct PaddleOcrConfig {
    pub base_url: String,
    pub api_token: String,
    pub model: String,
    pub poll_interval_secs: u64,
    pub job_timeout_secs: u64,
    pub max_jobs_per_document: usize,
    pub max_concurrent_jobs: usize,
    pub max_file_size_bytes: u64,
    pub max_image_size_bytes: u64,
}

/// Alias for architecture doc naming.
pub type PaddleJobsOcrService = PaddleOcrClient;

impl PaddleOcrConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            base_url: std::env::var("PADDLE_OCR_BASE_URL")
                .unwrap_or_else(|_| "https://paddleocr.aistudio-app.com/api/v2/ocr".to_string()),
            api_token: std::env::var("PADDLE_OCR_API_TOKEN")
                .context("PADDLE_OCR_API_TOKEN not set")?,
            model: std::env::var("PADDLE_OCR_MODEL")
                .unwrap_or_else(|_| "PaddleOCR-VL-1.6".to_string()),
            poll_interval_secs: std::env::var("PADDLE_OCR_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            job_timeout_secs: std::env::var("PADDLE_OCR_JOB_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            max_jobs_per_document: std::env::var("PADDLE_OCR_MAX_JOBS_PER_DOCUMENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            max_concurrent_jobs: std::env::var("PADDLE_OCR_MAX_CONCURRENT_JOBS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            max_file_size_bytes: std::env::var("PADDLE_OCR_MAX_FILE_SIZE_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_FILE_SIZE_BYTES),
            max_image_size_bytes: std::env::var("PADDLE_OCR_MAX_IMAGE_SIZE_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_IMAGE_SIZE_BYTES),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct SubmitJobData {
    #[serde(rename = "jobId")]
    job_id: String,
}

#[derive(Debug, Deserialize)]
struct JobStatusData {
    state: String,
    #[serde(rename = "resultUrl", default)]
    result_url: Option<ResultUrl>,
}

#[derive(Debug, Deserialize)]
struct ResultUrl {
    #[serde(rename = "jsonUrl")]
    json_url: Option<String>,
}

/// JSONL line wrapper: each line may be `{ "result": { "layoutParsingResults": [...] } }`
/// or directly `{ "layoutParsingResults": [...] }`.
#[derive(Debug, Deserialize)]
struct JsonlLine {
    #[serde(default)]
    result: Option<LayoutResult>,
    #[serde(rename = "layoutParsingResults", default)]
    layout_parsing_results: Vec<LayoutParsingResult>,
}

#[derive(Debug, Deserialize)]
struct LayoutResult {
    #[serde(rename = "layoutParsingResults", default)]
    layout_parsing_results: Vec<LayoutParsingResult>,
}

#[derive(Debug, Deserialize)]
struct LayoutParsingResult {
    #[serde(default)]
    markdown: Option<MarkdownContent>,
}

#[derive(Debug, Deserialize)]
struct MarkdownContent {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    images: BTreeMap<String, String>,
}

/// A figure extracted from PaddleOCR layout results.
#[derive(Debug, Clone)]
pub struct PaddleOcrFigure {
    pub image_key: String,
    pub image_url: String,
    pub surrounding_text: String,
}

/// Per-page OCR output from PaddleOCR.
#[derive(Debug, Clone)]
pub struct PaddleOcrPageResult {
    pub page_number: u32,
    pub text: String,
    pub figures: Vec<PaddleOcrFigure>,
}

pub struct PaddleOcrClient {
    config: PaddleOcrConfig,
    http: Client,
}

impl PaddleOcrClient {
    pub fn new(config: PaddleOcrConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .no_proxy()
            .build()
            .expect("failed to build HTTP client");
        Self { config, http }
    }

    pub async fn ocr_pdf_bytes(
        &self,
        pdf_bytes: &[u8],
        start_page: u32,
    ) -> Result<Vec<PaddleOcrPageResult>, IngestionError> {
        if pdf_bytes.len() as u64 > self.config.max_file_size_bytes {
            return Err(IngestionError::ocr_rejected(format!(
                "file size {} bytes exceeds Paddle OCR max file size {} bytes",
                pdf_bytes.len(),
                self.config.max_file_size_bytes
            )));
        }
        self.run_ocr_job(pdf_bytes, "document.pdf", "application/pdf", start_page)
            .await
    }

    pub async fn ocr_image_bytes(
        &self,
        image_bytes: &[u8],
        filename: &str,
    ) -> Result<PaddleOcrPageResult, IngestionError> {
        if image_bytes.len() as u64 > self.config.max_image_size_bytes {
            return Err(IngestionError::ocr_rejected(format!(
                "image size {} bytes exceeds Paddle OCR max image size {} bytes",
                image_bytes.len(),
                self.config.max_image_size_bytes
            )));
        }
        let (mime_type, upload_name) = image_upload_meta(filename)?;
        let pages = self
            .run_ocr_job(image_bytes, upload_name, mime_type, 1)
            .await?;
        pages.into_iter().next().ok_or_else(|| {
            IngestionError::parse(format!(
                "PaddleOCR returned no pages for image {filename}"
            ))
        })
    }

    async fn run_ocr_job(
        &self,
        file_bytes: &[u8],
        upload_name: &str,
        mime_type: &str,
        start_page: u32,
    ) -> Result<Vec<PaddleOcrPageResult>, IngestionError> {
        let job_id = self.submit_job(file_bytes, upload_name, mime_type).await?;
        info!(job_id = %job_id, start_page, "PaddleOCR job submitted");

        let result_url = self.poll_job(&job_id).await?;
        let json_url = result_url.json_url.ok_or_else(|| {
            IngestionError::parse("PaddleOCR job done but no jsonUrl".to_string())
        })?;

        let pages = self.fetch_and_parse_result(&json_url, start_page).await?;
        Ok(pages)
    }

    async fn submit_job(
        &self,
        file_bytes: &[u8],
        upload_name: &str,
        mime_type: &str,
    ) -> Result<String, IngestionError> {
        let url = format!("{}/jobs", self.config.base_url);
        let optional_payload = optional_payload_json();

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes.to_vec())
                    .file_name(upload_name.to_string())
                    .mime_str(mime_type)
                    .map_err(|e| IngestionError::parse(format!("PaddleOCR mime error: {e}")))?,
            )
            .text("model", self.config.model.clone())
            .text("optionalPayload", optional_payload.to_string());

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .multipart(form)
            .send()
            .await
            .map_err(|e| IngestionError::parse(format!("PaddleOCR submit request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            // Deterministic 4xx (bad auth, bad file, too large, …) → terminal; retrying
            // a 4xx cannot succeed. Transient 5xx/408/429 → retryable parse error.
            return Err(submit_error(status, &body));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| IngestionError::parse(format!("reading submit response body: {e}")))?;
        let resp: ApiResponse<SubmitJobData> = serde_json::from_str(&body)
            .map_err(|e| IngestionError::parse(format!("invalid submit response JSON: {e}")))?;
        Ok(resp.data.job_id)
    }

    async fn poll_job(&self, job_id: &str) -> Result<ResultUrl, IngestionError> {
        let url = format!("{}/jobs/{}", self.config.base_url, job_id);
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(self.config.job_timeout_secs);
        let mut consecutive_non_ok = 0u32;

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(IngestionError::Timeout(self.config.job_timeout_secs));
            }

            let resp = self
                .http
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_token))
                .send()
                .await
                .map_err(|e| IngestionError::parse(format!("PaddleOCR poll request failed: {e}")))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(job_id, %status, body, "PaddleOCR poll non-200");
                // Deterministic 4xx → fail now, never silently poll through it.
                if is_deterministic_rejection(status) {
                    return Err(IngestionError::ocr_rejected(format!(
                        "PaddleOCR poll failed ({status}): {body}"
                    )));
                }
                // Transient (5xx/408/429): bounded retries, then fail — no unbounded wait.
                consecutive_non_ok += 1;
                if consecutive_non_ok >= MAX_CONSECUTIVE_POLL_FAILURES {
                    return Err(IngestionError::parse(format!(
                        "PaddleOCR poll failed after {consecutive_non_ok} consecutive non-200 responses (last {status}): {body}"
                    )));
                }
                sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                continue;
            }

            consecutive_non_ok = 0;
            let body = resp
                .text()
                .await
                .map_err(|e| IngestionError::parse(format!("reading poll response body: {e}")))?;
            let resp: ApiResponse<JobStatusData> = serde_json::from_str(&body)
                .map_err(|e| IngestionError::parse(format!("invalid poll response JSON: {e}")))?;
            let status = resp.data;
            debug!(job_id, state = %status.state, "PaddleOCR poll");

            match status.state.as_str() {
                "done" | "success" | "completed" => {
                    return status
                        .result_url
                        .ok_or_else(|| IngestionError::parse("job done but no result_url".to_string()));
                }
                "failed" | "error" => {
                    return Err(IngestionError::parse(format!(
                        "PaddleOCR job {job_id} failed (state={})",
                        status.state
                    )));
                }
                _ => {
                    sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                }
            }
        }
    }

    async fn fetch_and_parse_result(
        &self,
        json_url: &str,
        start_page: u32,
    ) -> Result<Vec<PaddleOcrPageResult>, IngestionError> {
        let resp = self
            .http
            .get(json_url)
            .send()
            .await
            .map_err(|e| IngestionError::parse(format!("fetch OCR result JSON failed: {e}")))?;

        let body = resp
            .text()
            .await
            .map_err(|e| IngestionError::parse(format!("reading OCR result body: {e}")))?;
        let pages = parse_jsonl_or_json_pages(&body, start_page)
            .map_err(|e| IngestionError::parse(format!("invalid OCR result JSON: {e}")))?;
        Ok(pages)
    }

    /// OCR a single-page PDF slice (1 page = 1 Job).
    pub async fn ocr_single_page_pdf(
        &self,
        pdf_bytes: &[u8],
        page_number: u32,
    ) -> Result<PaddleOcrPageResult, IngestionError> {
        let pages = self.ocr_pdf_bytes(pdf_bytes, page_number).await?;
        pages.into_iter().next().ok_or_else(|| {
            IngestionError::parse(format!("PaddleOCR returned no pages for page {page_number}"))
        })
    }
}

/// Map a submit HTTP status to an `IngestionError`, marking deterministic 4xx
/// as terminal (`OcrRejected`) and transient 5xx/408/429 as retryable (`Parse`).
fn submit_error(status: StatusCode, body: &str) -> IngestionError {
    let message = format!("PaddleOCR submit failed ({status}): {body}");
    if is_deterministic_rejection(status) {
        IngestionError::ocr_rejected(message)
    } else {
        IngestionError::parse(message)
    }
}

/// Deterministic provider rejection that retrying cannot fix: any 4xx except
/// the retryable 408 (timeout) / 429 (rate-limit).
fn is_deterministic_rejection(status: StatusCode) -> bool {
    status.is_client_error()
        && status != StatusCode::REQUEST_TIMEOUT
        && status != StatusCode::TOO_MANY_REQUESTS
}

/// JSON payload for Paddle optionalPayload (§7.2).
pub fn optional_payload_json() -> serde_json::Value {
    let use_doc_orientation = std::env::var("PADDLE_OCR_USE_DOC_ORIENTATION_CLASSIFY")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(true);
    serde_json::json!({
        "useDocOrientationClassify": use_doc_orientation,
        "useDocUnwarping": true,
        "useChartRecognition": false,
    })
}

/// Stable sha256 hex digest of the optional payload JSON for cache keys.
pub fn optional_payload_hash() -> String {
    use sha2::{Digest, Sha256};
    let payload = optional_payload_json().to_string();
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn image_upload_meta(filename: &str) -> Result<(&'static str, &'static str), IngestionError> {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok(("image/png", "document.png")),
        "jpg" | "jpeg" => Ok(("image/jpeg", "document.jpg")),
        "webp" => Ok(("image/webp", "document.webp")),
        "gif" => Ok(("image/gif", "document.gif")),
        "bmp" => Ok(("image/bmp", "document.bmp")),
        other => Err(IngestionError::parse(format!(
            "unsupported image extension for Paddle OCR: {other}"
        ))),
    }
}

fn parse_jsonl_or_json_pages(
    body: &str,
    start_page: u32,
) -> anyhow::Result<Vec<PaddleOcrPageResult>> {
    let mut pages = Vec::new();

    for (i, line) in body.trim().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: JsonlLine =
            serde_json::from_str(line).context("parse OCR result line failed")?;

        let layouts = if let Some(result) = parsed.result {
            result.layout_parsing_results
        } else {
            parsed.layout_parsing_results
        };

        let mut page_text_parts = Vec::new();
        let mut page_figures = Vec::new();

        for layout in &layouts {
            if let Some(md) = &layout.markdown {
                if let Some(text) = &md.text {
                    if !text.trim().is_empty() {
                        page_text_parts.push(text.clone());
                    }
                }
                for (key, url) in &md.images {
                    let surrounding = layout
                        .markdown
                        .as_ref()
                        .and_then(|m| m.text.as_deref())
                        .unwrap_or("")
                        .to_string();
                    page_figures.push(PaddleOcrFigure {
                        image_key: key.clone(),
                        image_url: url.clone(),
                        surrounding_text: surrounding,
                    });
                }
            }
        }

        pages.push(PaddleOcrPageResult {
            page_number: start_page + i as u32,
            text: page_text_parts.join("\n\n"),
            figures: page_figures,
        });
    }

    Ok(pages)
}

/// Assemble DocumentIr from Paddle page results (co-located with OCR client).
pub fn build_document_ir_from_paddle(
    document_id: uuid::Uuid,
    filename: &str,
    pages: &[PaddleOcrPageResult],
    table_ocr_pages: &std::collections::HashSet<u32>,
) -> crate::ir::DocumentIr {
    use crate::ir::{
        AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
        ParseBackend, SourceLocator,
    };
    use std::collections::BTreeMap;

    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    ir.metadata
        .insert("ocr_backend".to_string(), "paddle_jobs".to_string());

    for page in pages {
        let is_table_page = table_ocr_pages.contains(&page.page_number);
        ir.pages.push(PageIr {
            page_number: page.page_number,
            width: None,
            height: None,
            backend: ParseBackend::PaddleOcrPdf,
            text_char_count: page.text.len(),
            image_count: page.figures.len(),
            metadata: Default::default(),
        });

        if !page.text.is_empty() {
            ir.blocks.push(BlockIr {
                block_id: format!("paddle-p{}-text", page.page_number),
                page: Some(page.page_number),
                block_type: if is_table_page {
                    BlockType::Table
                } else {
                    BlockType::Paragraph
                },
                modality: BlockModality::TextOnly,
                text: page.text.clone(),
                alt_text: None,
                asset_refs: Vec::new(),
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page.page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: Default::default(),
            });
        }

        for (fig_idx, figure) in page.figures.iter().enumerate() {
            let asset_id = format!("paddle-p{}-fig{}", page.page_number, fig_idx);
            let mut asset_metadata = BTreeMap::new();
            asset_metadata.insert("source".to_string(), "paddle_ocr".to_string());
            asset_metadata.insert("ephemeral_url".to_string(), "true".to_string());
            asset_metadata.insert("original_url".to_string(), figure.image_url.clone());
            ir.assets.push(AssetIr {
                asset_id: asset_id.clone(),
                page: Some(page.page_number),
                asset_kind: AssetKind::Image,
                storage_path: figure.image_url.clone(),
                mime_type: None,
                width: None,
                height: None,
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: asset_metadata,
            });

            ir.blocks.push(BlockIr {
                block_id: format!("paddle-p{}-fig{}", page.page_number, fig_idx),
                page: Some(page.page_number),
                block_type: BlockType::Figure,
                modality: BlockModality::ImageWithContext,
                text: figure.surrounding_text.clone(),
                alt_text: Some(figure.image_key.clone()),
                asset_refs: vec![asset_id],
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page.page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::PaddleOcrPdf,
                metadata: BTreeMap::from([(
                    "paddle_image_key".to_string(),
                    figure.image_key.clone(),
                )]),
            });
        }
    }

    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_upload_meta_maps_common_extensions() {
        assert_eq!(
            image_upload_meta("photo.PNG").unwrap(),
            ("image/png", "document.png")
        );
        assert_eq!(
            image_upload_meta("scan.jpeg").unwrap(),
            ("image/jpeg", "document.jpg")
        );
        assert!(image_upload_meta("file.xyz").is_err());
    }

    #[test]
    fn optional_payload_hash_changes_with_orientation_flag() {
        unsafe {
            std::env::remove_var("PADDLE_OCR_USE_DOC_ORIENTATION_CLASSIFY");
        }
        let default_hash = optional_payload_hash();
        unsafe {
            std::env::set_var("PADDLE_OCR_USE_DOC_ORIENTATION_CLASSIFY", "false");
        }
        let disabled_hash = optional_payload_hash();
        unsafe {
            std::env::remove_var("PADDLE_OCR_USE_DOC_ORIENTATION_CLASSIFY");
        }
        assert_ne!(default_hash, disabled_hash);
        let payload = optional_payload_json();
        assert_eq!(
            payload
                .get("useDocOrientationClassify")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_parse_jsonl_with_result_wrapper() {
        let body = r#"{"result": {"layoutParsingResults": [{"markdown": {"text": "Hello world", "images": {"img1.jpg": "https://example.com/img1.jpg"}}}]}}"#;
        let pages = parse_jsonl_or_json_pages(body, 1).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[0].text, "Hello world");
        assert_eq!(pages[0].figures.len(), 1);
        assert_eq!(pages[0].figures[0].image_key, "img1.jpg");
    }

    #[test]
    fn test_parse_jsonl_direct_layout() {
        let body =
            r#"{"layoutParsingResults": [{"markdown": {"text": "Page text", "images": {}}}]}"#;
        let pages = parse_jsonl_or_json_pages(body, 5).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 5);
        assert_eq!(pages[0].text, "Page text");
        assert!(pages[0].figures.is_empty());
    }

    #[test]
    fn test_parse_jsonl_multiple_lines() {
        let body = r#"{"layoutParsingResults": [{"markdown": {"text": "P1", "images": {}}}]}
{"layoutParsingResults": [{"markdown": {"text": "P2", "images": {}}}]}"#;
        let pages = parse_jsonl_or_json_pages(body, 10).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_number, 10);
        assert_eq!(pages[1].page_number, 11);
    }

    #[test]
    fn test_submit_response_nested_data() {
        let body = r#"{"data": {"jobId": "58419290363367424"}}"#;
        let resp: ApiResponse<SubmitJobData> = serde_json::from_str(body).unwrap();
        assert_eq!(resp.data.job_id, "58419290363367424");
    }

    #[test]
    fn test_poll_response_nested_data() {
        let body = r#"{"data": {"state": "done", "resultUrl": {"jsonUrl": "https://example.com/result.json"}}}"#;
        let resp: ApiResponse<JobStatusData> = serde_json::from_str(body).unwrap();
        assert_eq!(resp.data.state, "done");
        assert_eq!(
            resp.data.result_url.unwrap().json_url.unwrap(),
            "https://example.com/result.json"
        );
    }

    #[test]
    fn test_poll_response_in_progress() {
        let body = r#"{"data": {"state": "processing"}}"#;
        let resp: ApiResponse<JobStatusData> = serde_json::from_str(body).unwrap();
        assert_eq!(resp.data.state, "processing");
        assert!(resp.data.result_url.is_none());
    }

    #[test]
    fn submit_error_classifies_4xx_terminal_and_5xx_retryable() {
        // Deterministic rejections → terminal (no retry).
        assert!(matches!(
            submit_error(StatusCode::BAD_REQUEST, "bad"),
            IngestionError::OcrRejected(_)
        ));
        assert!(matches!(
            submit_error(StatusCode::UNAUTHORIZED, "no"),
            IngestionError::OcrRejected(_)
        ));
        assert!(matches!(
            submit_error(StatusCode::NOT_FOUND, "nf"),
            IngestionError::OcrRejected(_)
        ));
        // Transient → retryable parse.
        assert!(matches!(
            submit_error(StatusCode::SERVICE_UNAVAILABLE, "down"),
            IngestionError::Parse { .. }
        ));
        assert!(matches!(
            submit_error(StatusCode::TOO_MANY_REQUESTS, "slow"),
            IngestionError::Parse { .. }
        ));
        assert!(matches!(
            submit_error(StatusCode::REQUEST_TIMEOUT, "t"),
            IngestionError::Parse { .. }
        ));
    }

    #[test]
    fn deterministic_rejection_predicate() {
        assert!(is_deterministic_rejection(StatusCode::BAD_REQUEST));
        assert!(is_deterministic_rejection(StatusCode::PAYLOAD_TOO_LARGE));
        assert!(is_deterministic_rejection(StatusCode::UNAUTHORIZED));
        assert!(!is_deterministic_rejection(StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_deterministic_rejection(StatusCode::REQUEST_TIMEOUT));
        assert!(!is_deterministic_rejection(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!is_deterministic_rejection(StatusCode::OK));
    }

    #[tokio::test]
    async fn oversized_input_rejected_before_any_submit() {
        // max_file_size_bytes = 10; base_url points at an unreachable host to prove
        // the size guard short-circuits before any network I/O.
        let config = PaddleOcrConfig {
            base_url: "http://127.0.0.1:9".to_string(),
            api_token: "unused".to_string(),
            model: "m".to_string(),
            poll_interval_secs: 1,
            job_timeout_secs: 60,
            max_jobs_per_document: 1,
            max_concurrent_jobs: 1,
            max_file_size_bytes: 10,
            max_image_size_bytes: 10,
        };
        let client = PaddleOcrClient::new(config);
        let err = client
            .ocr_pdf_bytes(&[0u8; 100], 1)
            .await
            .expect_err("oversized input must fail before submit");
        assert!(
            matches!(err, IngestionError::OcrRejected(_)),
            "oversized must be terminal OcrRejected, got: {err}"
        );
    }

    #[tokio::test]
    async fn oversized_image_rejected_before_any_submit() {
        let config = PaddleOcrConfig {
            base_url: "http://127.0.0.1:9".to_string(),
            api_token: "unused".to_string(),
            model: "m".to_string(),
            poll_interval_secs: 1,
            job_timeout_secs: 60,
            max_jobs_per_document: 1,
            max_concurrent_jobs: 1,
            max_file_size_bytes: 200 * 1024 * 1024,
            max_image_size_bytes: 10,
        };
        let client = PaddleOcrClient::new(config);
        // 100 bytes image exceeds the 10-byte image cap → rejected before upload.
        let err = client
            .ocr_image_bytes(&[0u8; 100], "photo.png")
            .await
            .expect_err("oversized image must fail before submit");
        assert!(
            matches!(err, IngestionError::OcrRejected(_)),
            "oversized image must be terminal OcrRejected, got: {err}"
        );
    }

    #[test]
    fn config_defaults_to_200mb_file_and_10mb_image_caps() {
        unsafe {
            std::env::remove_var("PADDLE_OCR_MAX_FILE_SIZE_BYTES");
            std::env::remove_var("PADDLE_OCR_MAX_IMAGE_SIZE_BYTES");
            std::env::set_var("PADDLE_OCR_API_TOKEN", "test-token");
        }
        let config = PaddleOcrConfig::from_env().expect("config with token");
        assert_eq!(config.max_file_size_bytes, DEFAULT_MAX_FILE_SIZE_BYTES);
        assert_eq!(config.max_image_size_bytes, DEFAULT_MAX_IMAGE_SIZE_BYTES);
        unsafe {
            std::env::set_var("PADDLE_OCR_MAX_IMAGE_SIZE_BYTES", "12345");
        }
        let config = PaddleOcrConfig::from_env().expect("config with token");
        assert_eq!(config.max_image_size_bytes, 12345);
        unsafe {
            std::env::remove_var("PADDLE_OCR_MAX_IMAGE_SIZE_BYTES");
        }
    }
}
