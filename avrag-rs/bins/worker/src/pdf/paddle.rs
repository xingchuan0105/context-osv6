use std::collections::{BTreeMap, HashSet};

use ingestion::parser::{PaddleOcrClient, PaddleOcrConfig, PaddleOcrPageResult};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, IngestionError,
    PageIr, ParseBackend, SourceLocator,
};
use uuid::Uuid;

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
) -> DocumentIr {
    let mut ir = DocumentIr::new(
        document_id.to_string(),
        filename.to_string(),
        DocumentType::Pdf,
        ParseBackend::PaddleOcrPdf,
    );
    ir.metadata
        .insert("ocr_backend".to_string(), "paddle".to_string());

    for page in pages {
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
                block_type: BlockType::Paragraph,
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

pub async fn execute_paddle_ocr(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    paddle_pages: &[u32],
) -> Result<(DocumentIr, HashSet<u32>), IngestionError> {
    let config = PaddleOcrConfig::from_env().map_err(|e| {
        IngestionError::StateSink(format!("PaddleOCR config error: {e}"))
    })?;
    let client = PaddleOcrClient::new(config);
    let batch_pages: usize = std::env::var("PADDLE_OCR_BATCH_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);

    let mut segments = group_contiguous_pages(paddle_pages);

    let max_single_jobs: usize = std::env::var("PADDLE_OCR_C_PRIME_MAX_SINGLE_JOBS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    if segments.len() > max_single_jobs {
        let mut merged_segments: Vec<(u32, u32)> = Vec::new();
        for seg in segments {
            if let Some(last) = merged_segments.last_mut() {
                if seg.0 - last.1 <= 2 {
                    last.1 = seg.1;
                    continue;
                }
            }
            merged_segments.push(seg);
        }
        while merged_segments.len() > max_single_jobs {
            let last = merged_segments.pop().unwrap();
            merged_segments.last_mut().unwrap().1 = last.1;
        }
        segments = merged_segments;
    }

    let mut all_results = Vec::new();

    for (seg_start, seg_end) in &segments {
        for chunk_start in (*seg_start..=*seg_end).step_by(batch_pages) {
            let chunk_end = (chunk_start + batch_pages as u32 - 1).min(*seg_end);
            let pdf_slice = extract_pdf_slice(bytes, chunk_start, chunk_end).map_err(|e| {
                IngestionError::StateSink(format!("PDF slice extraction failed: {e}"))
            })?;

            let results = client
                .ocr_pdf_bytes(&pdf_slice, chunk_start)
                .await
                .map_err(|e| IngestionError::StateSink(format!("PaddleOCR failed: {e}")))?;

            all_results.extend(results);
        }
    }

    let successful_pages: HashSet<u32> = all_results
        .iter()
        .filter(|r| !r.text.is_empty())
        .map(|r| r.page_number)
        .collect();
    Ok((
        build_document_ir_from_paddle(document_id, filename, &all_results),
        successful_pages,
    ))
}
