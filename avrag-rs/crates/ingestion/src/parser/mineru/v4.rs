use anyhow::{Context, Result};
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

use super::config::{
    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS, DEFAULT_POLL_INTERVAL_SECS, ExtractV4BatchData,
    ExtractV4CreateTaskData, ExtractV4Envelope, ExtractV4TaskData, ExtractV4UploadUrlData,
};
use super::figure::ImageInfo;
use super::table::{
    build_file_upload_batch_payload_v4, build_file_upload_batch_payload_v4_files,
    extract_markdown_and_images_from_zip, format_page_ranges,
};
use super::upload::prepare_v4_file_upload_payload;
use super::{MineruClient, NormalizedDocument};

impl MineruClient {
    pub(crate) async fn parse_with_page_filter_v4_remote(
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

    pub(crate) async fn parse_with_page_filter_v4_upload(
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
    pub(crate) async fn create_file_upload_batch_v4(
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

    pub(crate) async fn create_file_upload_batch_v4_files(
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

    pub(crate) async fn upload_file_to_signed_url_v4(&self, upload_url: &str, bytes: &[u8]) -> Result<()> {
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

    pub(crate) async fn create_extract_task_v4(
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

    pub(crate) async fn wait_for_completion_v4(&self, task_id: &str) -> Result<ExtractV4TaskData> {
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

    pub(crate) async fn wait_for_batch_completion_v4(
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

    pub(crate) async fn fetch_zip_payload_v4(
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

    pub(crate) async fn fetch_markdown(&self, url: &str) -> Result<String> {
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
}
