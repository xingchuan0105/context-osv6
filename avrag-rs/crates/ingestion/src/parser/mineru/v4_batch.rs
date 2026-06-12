use std::collections::BTreeMap;
use anyhow::{Context, Result};
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

use super::config::{
    DEFAULT_MINERU_TASK_TIMEOUT_ATTEMPTS, DEFAULT_POLL_INTERVAL_SECS, ExtractV4BatchData,
    ExtractV4Envelope, ExtractV4TaskData, MINERU_V4_MAX_FILES_PER_UPLOAD_BATCH,
    MINERU_V4_UPLOAD_BATCH_COOLDOWN,
};
use super::fallback::{is_low_value_ocr_document, skipped_ocr_page_unit};
use super::upload::{prepare_v4_ocr_page_upload, MineruV4PageUploadFile};
use super::{MineruClient, NormalizedDocument};

impl MineruClient {
    pub(crate) async fn parse_pdf_pages_v4_upload_batch(
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

        let mut units = skipped_before_upload
            .iter()
            .map(|page_number| skipped_ocr_page_unit(*page_number))
            .collect::<Vec<_>>();
        let mut title: Option<String> = None;
        let mut skipped_after_ocr = 0usize;
        let mut total_uploaded = 0usize;
        let file_chunks: Vec<&[MineruV4PageUploadFile]> = files
            .chunks(MINERU_V4_MAX_FILES_PER_UPLOAD_BATCH)
            .collect();

        for (chunk_index, file_chunk) in file_chunks.iter().enumerate() {
            if chunk_index > 0 {
                info!(
                    filename,
                    chunk_index,
                    chunk_count = file_chunks.len(),
                    "MinerU v4 OCR batch rate-limit cooldown before next upload batch"
                );
                sleep(MINERU_V4_UPLOAD_BATCH_COOLDOWN).await;
            }

            let filenames = file_chunk
                .iter()
                .map(|file| file.filename.clone())
                .collect::<Vec<_>>();
            let (batch_id, upload_urls) = self
                .create_file_upload_batch_v4_files(&filenames, true)
                .await?;
            if upload_urls.len() != file_chunk.len() {
                anyhow::bail!(
                    "MinerU v4 file upload batch returned {} upload URLs for {} files",
                    upload_urls.len(),
                    file_chunk.len()
                );
            }
            info!(
                batch_id,
                filename,
                chunk_index,
                chunk_count = file_chunks.len(),
                file_count = file_chunk.len(),
                skipped_before_upload = skipped_before_upload.len(),
                "MinerU v4 OCR batch upload URLs created"
            );

            for (file, upload_url) in file_chunk.iter().zip(upload_urls.iter()) {
                self.upload_file_to_signed_url_v4(upload_url, &file.bytes)
                    .await
                    .with_context(|| {
                        format!("Failed to upload MinerU OCR page {}", file.page_number)
                    })?;
            }
            info!(
                batch_id,
                filename,
                chunk_index,
                file_count = file_chunk.len(),
                "MinerU v4 OCR batch files uploaded, waiting for batch result"
            );

            let results = self
                .wait_for_batch_completion_v4_all(&batch_id, file_chunk.len())
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

            for file in *file_chunk {
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

            total_uploaded += file_chunk.len();
        }

        info!(
            filename,
            pages = page_numbers.len(),
            uploaded = total_uploaded,
            upload_batches = file_chunks.len(),
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
    pub(crate) async fn wait_for_batch_completion_v4_all(
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

}
