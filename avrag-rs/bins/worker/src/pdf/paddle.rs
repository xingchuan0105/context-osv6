use std::collections::{BTreeMap, HashSet};

use ingestion::parser::{
    PaddleOcrClient, PaddleOcrConfig, PaddleOcrPageResult, PaddleResultCache, optional_payload_hash,
};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, IngestionError, PageIr,
    ParseBackend, SourceLocator,
};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

/// Outcome of per-page Paddle Jobs OCR (LiteParse path).
#[derive(Debug, Clone)]
pub struct PaddlePerPageOcrOutcome {
    pub ir: DocumentIr,
    pub successful_pages: HashSet<u32>,
    /// Pages where the Paddle Job/API failed (per-page tolerance).
    pub failed_pages: Vec<u32>,
    /// Pages not submitted because `PADDLE_OCR_MAX_JOBS_PER_DOCUMENT` was reached.
    pub budget_skipped_pages: Vec<u32>,
    /// Jobs actually submitted to the Paddle API (cache hits excluded).
    pub jobs_submitted: usize,
    /// Total pages satisfied from cache (no API call).
    pub cache_hits: usize,
}

/// Pages that would remain after the job budget is exhausted at `start_index`.
pub fn pages_remaining_after_budget(paddle_pages: &[u32], start_index: usize) -> Vec<u32> {
    paddle_pages.iter().skip(start_index).copied().collect()
}

#[cfg(test)]
pub fn group_contiguous_pages(pages: &[u32]) -> Vec<(u32, u32)> {
    if pages.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut start = pages[0];
    let mut end = pages[0];

    for &page in &pages[1..] {
        if page == end + 1 {
            end = page;
        } else {
            segments.push((start, end));
            start = page;
            end = page;
        }
    }
    segments.push((start, end));
    segments
}

pub fn extract_pdf_slice(bytes: &[u8], start_page: u32, end_page: u32) -> anyhow::Result<Vec<u8>> {
    let mut doc = lopdf::Document::load_mem(bytes)?;
    let all_pages = doc.get_pages();
    let total_pages = all_pages.len();
    let pages_to_remove: Vec<u32> = all_pages
        .into_iter()
        .filter(|(num, _)| *num < start_page || *num > end_page)
        .map(|(num, _)| num)
        .collect();

    if pages_to_remove.len() == total_pages {
        anyhow::bail!("no pages in range {start_page}-{end_page}");
    }

    doc.delete_pages(&pages_to_remove);
    doc.renumber_objects();

    let mut buf = Vec::new();
    doc.save_to(&mut buf)?;
    Ok(buf)
}

pub fn build_document_ir_from_paddle(
    document_id: Uuid,
    filename: &str,
    pages: &[PaddleOcrPageResult],
    table_ocr_pages: &HashSet<u32>,
) -> DocumentIr {
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
                asset_kind: ingestion::AssetKind::Image,
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

fn page_is_successful(result: &PaddleOcrPageResult) -> bool {
    !result.text.trim().is_empty() || !result.figures.is_empty()
}

async fn ocr_single_page_with_retry(
    client: &PaddleOcrClient,
    pdf_slice: &[u8],
    page_number: u32,
    filename: &str,
) -> Result<PaddleOcrPageResult, ingestion::IngestionError> {
    match client
        .ocr_single_page_pdf(pdf_slice, page_number)
        .await
        .map_err(|e| IngestionError::StateSink(format!("PaddleOCR page job failed: {e}")))
    {
        Ok(result) => Ok(result),
        Err(first_err) => {
            tracing::warn!(
                metric = "paddle_jobs_failed",
                page_number,
                filename,
                error = %first_err,
                "paddle job failed; retrying once"
            );
            sleep(Duration::from_secs(5)).await;
            client
                .ocr_single_page_pdf(pdf_slice, page_number)
                .await
                .map_err(|e| {
                    IngestionError::StateSink(format!(
                        "PaddleOCR page job failed after retry: {e} (first: {first_err})"
                    ))
                })
        }
    }
}

/// LiteParse path: 1 page = 1 Paddle Job with optional result cache.
pub async fn execute_paddle_ocr_per_page(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    paddle_pages: &[u32],
    table_ocr_pages: &HashSet<u32>,
) -> Result<PaddlePerPageOcrOutcome, IngestionError> {
    let config = PaddleOcrConfig::from_env()
        .map_err(|e| IngestionError::StateSink(format!("PaddleOCR config error: {e}")))?;
    let client = PaddleOcrClient::new(config.clone());
    let mut cache = PaddleResultCache::from_env();
    let payload_hash = optional_payload_hash();
    let mut all_results = Vec::new();
    let mut jobs_submitted = 0usize;
    let mut cache_hits = 0usize;
    let mut failed_pages = Vec::new();
    let mut budget_skipped_pages = Vec::new();

    for (idx, &page_number) in paddle_pages.iter().enumerate() {
        if jobs_submitted >= config.max_jobs_per_document {
            budget_skipped_pages.extend(pages_remaining_after_budget(paddle_pages, idx));
            tracing::warn!(
                metric = "paddle_jobs_budget_exhausted",
                filename,
                page_number,
                max = config.max_jobs_per_document,
                skipped = budget_skipped_pages.len(),
                "Paddle Job budget exhausted for document"
            );
            break;
        }

        let pdf_slice = match extract_pdf_slice(bytes, page_number, page_number) {
            Ok(slice) => slice,
            Err(e) => {
                tracing::warn!(
                    metric = "paddle_pdf_slice_failed",
                    page_number,
                    filename,
                    error = %e,
                    "PDF slice extraction failed for paddle page"
                );
                failed_pages.push(page_number);
                continue;
            }
        };
        let cache_key =
            PaddleResultCache::cache_key(&pdf_slice, page_number, &config.model, &payload_hash);
        if let Some(cached) = cache.get(&cache_key) {
            tracing::info!(
                metric = "paddle_cache_hits",
                page_number,
                filename,
                cache_hits,
                "paddle job cache hit"
            );
            all_results.extend(cached);
            cache_hits += 1;
            continue;
        }

        tracing::info!(
            metric = "paddle_jobs_submitted",
            page_number,
            filename,
            jobs_submitted,
            max_jobs = config.max_jobs_per_document,
            "paddle job submitted"
        );
        let started = std::time::Instant::now();
        match ocr_single_page_with_retry(&client, &pdf_slice, page_number, filename).await {
            Ok(page_result) => {
                tracing::info!(
                    metric = "paddle_jobs_latency_ms",
                    page_number,
                    filename,
                    latency_ms = started.elapsed().as_millis() as u64,
                    "paddle job completed"
                );
                cache.put(cache_key, vec![page_result.clone()]);
                all_results.push(page_result);
                jobs_submitted += 1;
            }
            Err(e) => {
                tracing::warn!(
                    metric = "paddle_jobs_failed",
                    page_number,
                    filename,
                    error = %e,
                    "paddle job failed after retry; continuing with remaining pages"
                );
                failed_pages.push(page_number);
            }
        }
    }

    tracing::info!(
        metric = "jobs_per_document",
        filename,
        jobs_submitted,
        cache_hits,
        failed_pages = failed_pages.len(),
        requested_pages = paddle_pages.len(),
        max_jobs = config.max_jobs_per_document,
        "paddle jobs finished for document"
    );

    let successful_pages: HashSet<u32> = all_results
        .iter()
        .filter(|r| page_is_successful(r))
        .map(|r| r.page_number)
        .collect();
    Ok(PaddlePerPageOcrOutcome {
        ir: build_document_ir_from_paddle(document_id, filename, &all_results, table_ocr_pages),
        successful_pages,
        failed_pages,
        budget_skipped_pages,
        jobs_submitted,
        cache_hits,
    })
}

/// Standalone image ingest: 1 file = 1 Paddle Job (page 1).
pub async fn execute_paddle_ocr_image(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
) -> Result<DocumentIr, IngestionError> {
    let config = PaddleOcrConfig::from_env()
        .map_err(|e| IngestionError::StateSink(format!("PaddleOCR config error: {e}")))?;
    let client = PaddleOcrClient::new(config);
    let page_result = client
        .ocr_image_bytes(bytes, filename)
        .await
        .map_err(|e| IngestionError::StateSink(format!("PaddleOCR image job failed: {e}")))?;

    let table_pages = HashSet::new();
    let mut ir = build_document_ir_from_paddle(
        document_id,
        filename,
        std::slice::from_ref(&page_result),
        &table_pages,
    );
    ir.doc_type = DocumentType::Image;
    ir.metadata.insert(
        "ingest_route_version".to_string(),
        "liteparse-v1".to_string(),
    );
    ir.metadata
        .insert("pdf_route_mode".to_string(), "paddle_image".to_string());
    ir.metadata
        .insert("paddle_jobs_requested".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_count".to_string(), "1".to_string());
    ir.metadata
        .insert("paddle_jobs_used".to_string(), "1".to_string());
    Ok(ir)
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn pages_remaining_after_budget_skips_from_index() {
        let pages = vec![1, 2, 3, 4, 5];
        assert_eq!(pages_remaining_after_budget(&pages, 0), pages);
        assert_eq!(pages_remaining_after_budget(&pages, 2), vec![3, 4, 5]);
        assert_eq!(pages_remaining_after_budget(&pages, 5), Vec::<u32>::new());
    }

    #[test]
    fn table_ocr_pages_emit_table_blocks() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 2,
            text: "| a | b |".to_string(),
            figures: vec![],
        }];
        let table_pages = HashSet::from([2]);
        let ir = build_document_ir_from_paddle(Uuid::new_v4(), "t.pdf", &pages, &table_pages);
        assert_eq!(ir.blocks.len(), 1);
        assert_eq!(ir.blocks[0].block_type, BlockType::Table);
        assert_eq!(
            ir.metadata.get("ocr_backend").map(String::as_str),
            Some("paddle_jobs")
        );
    }

    /// Documents metadata contract for standalone image ingest (`execute_paddle_ocr_image`).
    #[test]
    fn paddle_image_route_metadata_contract() {
        let pages = vec![PaddleOcrPageResult {
            page_number: 1,
            text: "image ocr text".to_string(),
            figures: vec![],
        }];
        let mut ir =
            build_document_ir_from_paddle(Uuid::new_v4(), "photo.png", &pages, &HashSet::new());
        ir.doc_type = DocumentType::Image;
        ir.metadata.insert(
            "ingest_route_version".to_string(),
            "liteparse-v1".to_string(),
        );
        ir.metadata
            .insert("pdf_route_mode".to_string(), "paddle_image".to_string());
        ir.metadata
            .insert("paddle_jobs_requested".to_string(), "1".to_string());
        ir.metadata
            .insert("paddle_jobs_count".to_string(), "1".to_string());
        ir.metadata
            .insert("paddle_jobs_used".to_string(), "1".to_string());

        assert_eq!(ir.doc_type, DocumentType::Image);
        assert_eq!(
            ir.metadata.get("pdf_route_mode").map(String::as_str),
            Some("paddle_image")
        );
        assert_eq!(
            ir.metadata.get("paddle_jobs_count").map(String::as_str),
            Some("1")
        );
        assert!(
            !ir.blocks.is_empty(),
            "image OCR should emit searchable text blocks"
        );
    }
}
