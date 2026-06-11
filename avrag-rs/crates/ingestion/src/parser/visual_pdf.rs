use anyhow::{Context, Result};
use uuid::Uuid;

use crate::ir::{
    AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
    ParseBackend, SourceLocator,
};

use super::pdf_renderer_service::{
    PdfRendererServiceClient, chunk_page_ranges, page_range_metadata, pages_per_visual_chunk,
    visual_render_strategy,
};

pub struct VisualPdfParser {
    client: PdfRendererServiceClient,
}

impl VisualPdfParser {
    pub fn new(client: PdfRendererServiceClient) -> Self {
        Self { client }
    }

    pub async fn parse_pages(
        &self,
        bytes: &[u8],
        filename: &str,
        document_id: Uuid,
        page_numbers: &[u32],
    ) -> Result<DocumentIr> {
        if page_numbers.is_empty() {
            return Ok(DocumentIr::new(
                document_id.to_string(),
                filename.to_string(),
                DocumentType::Pdf,
                ParseBackend::VisualRasterPdf,
            ));
        }

        let chunk_size = pages_per_visual_chunk();
        let strategy = visual_render_strategy();
        let mut document = DocumentIr::new(
            document_id.to_string(),
            filename.to_string(),
            DocumentType::Pdf,
            ParseBackend::VisualRasterPdf,
        );
        document
            .metadata
            .insert("ingest_route".to_string(), "visual".to_string());
        document.metadata.insert(
            "visual_pages_per_chunk".to_string(),
            chunk_size.to_string(),
        );

        let mut sorted_pages = page_numbers.to_vec();
        sorted_pages.sort_unstable();
        sorted_pages.dedup();

        for (range_start, range_end) in chunk_page_ranges(&sorted_pages, chunk_size) {
            let rendered = self
                .client
                .render_pages(bytes, filename, range_start, range_end, &strategy)
                .await
                .with_context(|| {
                    format!("render pdf pages {range_start}-{range_end} for {filename}")
                })?;

            let mut asset_refs = Vec::new();
            let mut page_numbers_in_chunk = Vec::new();
            for page in rendered.pages {
                let asset_id = Uuid::new_v4().to_string();
                let temp_path = write_temp_jpeg(&page.image_base64, &asset_id)?;
                asset_refs.push(asset_id.clone());
                page_numbers_in_chunk.push(page.page_number);

                let mut asset_meta = page_range_metadata(range_start, range_end);
                asset_meta.insert("render_strategy".to_string(), strategy.clone());

                document.assets.push(AssetIr {
                    asset_id: asset_id.clone(),
                    page: Some(page.page_number),
                    asset_kind: AssetKind::Image,
                    storage_path: temp_path,
                    mime_type: Some(page.mime_type),
                    width: Some(page.width),
                    height: Some(page.height),
                    parser_backend: ParseBackend::VisualRasterPdf,
                    metadata: asset_meta,
                });
            }

            if asset_refs.is_empty() {
                continue;
            }

            let caption = if range_start == range_end {
                format!("PDF page {range_start}")
            } else {
                format!("PDF pages {range_start}-{range_end}")
            };

            let mut block_meta = page_range_metadata(range_start, range_end);
            block_meta.insert(
                "page_numbers".to_string(),
                page_numbers_in_chunk
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );

            let block_id = Uuid::new_v4().to_string();
            document.pages.push(PageIr {
                page_number: range_start,
                backend: ParseBackend::VisualRasterPdf,
                text_char_count: 0,
                image_count: asset_refs.len(),
                ..Default::default()
            });
            document.blocks.push(BlockIr {
                block_id: block_id.clone(),
                page: Some(range_start),
                block_type: BlockType::PageRaster,
                modality: BlockModality::ImageWithContext,
                text: String::new(),
                alt_text: Some(caption.clone()),
                asset_refs,
                caption: Some(caption),
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(range_start),
                    ..Default::default()
                },
                parser_backend: ParseBackend::VisualRasterPdf,
                metadata: block_meta,
            });
        }

        Ok(document)
    }
}

fn write_temp_jpeg(image_base64: &str, asset_id: &str) -> Result<String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_base64)
        .context("decode page jpeg base64")?;
    let path = std::env::temp_dir().join(format!("avrag-page-raster-{asset_id}.jpg"));
    std::fs::write(&path, bytes).context("write temp page jpeg")?;
    Ok(format!("temporary://{}", path.display()))
}
