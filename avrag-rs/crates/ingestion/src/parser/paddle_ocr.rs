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
}

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
        let job_id = self.submit_job(pdf_bytes).await?;
        info!(job_id = %job_id, start_page, "PaddleOCR job submitted");

        let result_url = self.poll_job(&job_id).await?;
        let json_url = result_url
            .json_url
            .context("PaddleOCR job done but no jsonUrl")?;

        let pages = self.fetch_and_parse_result(&json_url, start_page).await?;
        Ok(pages)
    }

    async fn submit_job(&self, pdf_bytes: &[u8]) -> Result<String> {
        let url = format!("{}/jobs", self.config.base_url);

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
                    .file_name("document.pdf")
                    .mime_str("application/pdf")?,
            )
            .text("model", self.config.model.clone())
            .text(
                "optionalPayload",
                serde_json::json!({
                    "useDocOrientationClassify": false,
                    "useDocUnwarping": true,
                    "useChartRecognition": false,
                })
                .to_string(),
            );

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
        let body = r#"{"layoutParsingResults": [{"markdown": {"text": "Page text", "images": {}}}]}"#;
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
