use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct PaddleOcrConfig {
    pub base_url: String,
    pub api_token: String,
    pub model: String,
    pub poll_interval_secs: u64,
    pub job_timeout_secs: u64,
    pub max_jobs_per_document: usize,
    pub max_concurrent_jobs: usize,
}

/// Alias for architecture doc naming.
pub type PaddleJobsOcrService = PaddleOcrClient;

impl PaddleOcrConfig {
    pub fn from_env() -> Result<Self> {
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
    ) -> Result<Vec<PaddleOcrPageResult>> {
        match self
            .run_ocr_job(pdf_bytes, "document.pdf", "application/pdf", start_page)
            .await
        {
            Ok(pages) => Ok(pages),
            Err(first_err) => {
                warn!(start_page, error = %first_err, "PaddleOCR job failed on first attempt, retrying after 5s");
                sleep(Duration::from_secs(5)).await;
                self.run_ocr_job(pdf_bytes, "document.pdf", "application/pdf", start_page)
                    .await
                    .map_err(|retry_err| {
                        retry_err.context(format!(
                            "PaddleOCR job failed after retry (first error: {first_err})"
                        ))
                    })
            }
        }
    }

    pub async fn ocr_image_bytes(
        &self,
        image_bytes: &[u8],
        filename: &str,
    ) -> Result<PaddleOcrPageResult> {
        let (mime_type, upload_name) = image_upload_meta(filename)?;
        match self
            .run_ocr_job(image_bytes, upload_name, mime_type, 1)
            .await
        {
            Ok(pages) => pages
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("PaddleOCR returned no pages for image {filename}")),
            Err(first_err) => {
                warn!(filename, error = %first_err, "PaddleOCR image job failed on first attempt, retrying after 5s");
                sleep(Duration::from_secs(5)).await;
                self.run_ocr_job(image_bytes, upload_name, mime_type, 1)
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "PaddleOCR returned no pages for image {filename} after retry"
                        )
                    })
            }
        }
    }

    async fn run_ocr_job(
        &self,
        file_bytes: &[u8],
        upload_name: &str,
        mime_type: &str,
        start_page: u32,
    ) -> Result<Vec<PaddleOcrPageResult>> {
        let job_id = self.submit_job(file_bytes, upload_name, mime_type).await?;
        info!(job_id = %job_id, start_page, "PaddleOCR job submitted");

        let result_url = self.poll_job(&job_id).await?;
        let json_url = result_url
            .json_url
            .context("PaddleOCR job done but no jsonUrl")?;

        let pages = self.fetch_and_parse_result(&json_url, start_page).await?;
        Ok(pages)
    }

    async fn submit_job(
        &self,
        file_bytes: &[u8],
        upload_name: &str,
        mime_type: &str,
    ) -> Result<String> {
        let url = format!("{}/jobs", self.config.base_url);
        let optional_payload = optional_payload_json();

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes.to_vec())
                    .file_name(upload_name.to_string())
                    .mime_str(mime_type)?,
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
            .context("PaddleOCR submit request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("PaddleOCR submit failed ({status}): {body}");
        }

        let body = resp.text().await.context("reading submit response body")?;
        let resp: ApiResponse<SubmitJobData> =
            serde_json::from_str(&body).context("invalid submit response JSON")?;
        Ok(resp.data.job_id)
    }

    async fn poll_job(&self, job_id: &str) -> Result<ResultUrl> {
        let url = format!("{}/jobs/{}", self.config.base_url, job_id);
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(self.config.job_timeout_secs);

        loop {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!(
                    "PaddleOCR job {job_id} timed out after {}s",
                    self.config.job_timeout_secs
                );
            }

            let resp = self
                .http
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_token))
                .send()
                .await
                .context("PaddleOCR poll request failed")?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(job_id, %status, body, "PaddleOCR poll non-200");
                sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                continue;
            }

            let body = resp.text().await.context("reading poll response body")?;
            let resp: ApiResponse<JobStatusData> =
                serde_json::from_str(&body).context("invalid poll response JSON")?;
            let status = resp.data;
            debug!(job_id, state = %status.state, "PaddleOCR poll");

            match status.state.as_str() {
                "done" | "success" | "completed" => {
                    return status.result_url.context("job done but no result_url");
                }
                "failed" | "error" => {
                    anyhow::bail!("PaddleOCR job {job_id} failed (state={})", status.state);
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
    ) -> Result<Vec<PaddleOcrPageResult>> {
        let resp = self
            .http
            .get(json_url)
            .send()
            .await
            .context("fetch OCR result JSON failed")?;

        let body = resp.text().await.context("reading OCR result body")?;
        let pages = parse_jsonl_or_json_pages(&body, start_page)?;
        Ok(pages)
    }

    /// OCR a single-page PDF slice (1 page = 1 Job).
    pub async fn ocr_single_page_pdf(
        &self,
        pdf_bytes: &[u8],
        page_number: u32,
    ) -> Result<PaddleOcrPageResult> {
        let pages = self.ocr_pdf_bytes(pdf_bytes, page_number).await?;
        pages
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("PaddleOCR returned no pages for page {page_number}"))
    }
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

fn image_upload_meta(filename: &str) -> Result<(&'static str, &'static str)> {
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
        other => anyhow::bail!("unsupported image extension for Paddle OCR: {other}"),
    }
}

fn parse_jsonl_or_json_pages(body: &str, start_page: u32) -> Result<Vec<PaddleOcrPageResult>> {
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
}
