use anyhow::{Context, Result};
use tokio::time::{sleep, Duration};
use tracing::debug;

use super::config::{DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS, DEFAULT_POLL_INTERVAL_SECS, LegacyTaskStatus, LegacyUploadResponse};
use super::{MineruClient, NormalizedDocument};

impl MineruClient {
    pub(crate) async fn parse_with_page_filter_legacy(
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
    pub(crate) async fn upload_file_legacy(
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

    pub(crate) async fn wait_for_completion_legacy(&self, task_id: &str) -> Result<()> {
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

    pub(crate) async fn fetch_result_legacy(
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
}
