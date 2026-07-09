use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient};
use ingestion::parser::{PageRouteKind, PdfParsePlan};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, LiteParseConfig, ParseBackend,
    SourceLocator,
};

use super::context::PdfParseContext;

pub async fn enrich_b_class_figures(
    ctx: &PdfParseContext,
    pdf_bytes: &[u8],
    ir: &mut DocumentIr,
    plan: &PdfParsePlan,
) {
    use ingestion::parser::{extract_page_images, image_mime_type, image_to_base64};

    let lp_config = LiteParseConfig::from_env();
    let decorative_max_area = lp_config.decorative_max_area;

    let b_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.route_kinds.contains(&PageRouteKind::Figure))
        .map(|p| p.page_number)
        .collect();

    if b_pages.is_empty() {
        return;
    }

    let llm = ctx.ingestion_llm.clone();

    for page_number in b_pages {
        let page_dims = ir
            .pages
            .iter()
            .find(|p| p.page_number == page_number)
            .and_then(|p| match (p.width, p.height) {
                (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some((w, h)),
                _ => None,
            });

        let images = match extract_page_images(pdf_bytes, page_number) {
            Ok(imgs) => imgs,
            Err(e) => {
                tracing::warn!(page = page_number, error = %e, "failed to extract page images");
                continue;
            }
        };

        for (fig_idx, img) in images.iter().enumerate() {
            if is_decorative_image(img, page_dims, decorative_max_area) {
                continue;
            }

            let asset_id = format!("bfig-p{}-{}", page_number, fig_idx);
            let mime = image_mime_type(img);
            let storage_path = match write_temp_figure(img, page_number, fig_idx) {
                Ok(path) => path,
                Err(e) => {
                    tracing::warn!(
                        page = page_number,
                        fig = fig_idx,
                        error = %e,
                        "failed to persist figure to temp storage"
                    );
                    continue;
                }
            };
            let base64_data = image_to_base64(img);
            let data_url = format!("data:{mime};base64,{base64_data}");

            let vlm_summary = if let Some(ref llm) = llm {
                match summarize_figure_with_vlm(llm, &data_url, page_number).await {
                    Ok(summary) if !summary.trim().is_empty() => Some(summary),
                    Ok(_) => None,
                    Err(e) => {
                        tracing::debug!(page = page_number, fig = fig_idx, error = %e, "VLM figure summary failed");
                        None
                    }
                }
            } else {
                None
            };

            let vlm_failed = vlm_summary.is_none();
            let figure_text = vlm_summary.unwrap_or_default();

            ir.assets.push(AssetIr {
                asset_id: asset_id.clone(),
                page: Some(page_number),
                asset_kind: ingestion::AssetKind::Image,
                storage_path,
                mime_type: Some(mime.to_string()),
                width: Some(img.width as u32),
                height: Some(img.height as u32),
                parser_backend: ParseBackend::LiteParseFigure,
                metadata: std::collections::BTreeMap::new(),
            });

            let mut metadata = std::collections::BTreeMap::new();
            if vlm_failed {
                metadata.insert("vlm_failed".to_string(), "true".to_string());
            } else {
                metadata.insert("vlm_summarized".to_string(), "true".to_string());
            }

            ir.blocks.push(BlockIr {
                block_id: format!("bfig-p{}-{}", page_number, fig_idx),
                page: Some(page_number),
                block_type: BlockType::Figure,
                modality: BlockModality::ImageWithContext,
                text: figure_text,
                alt_text: Some(format!("Figure from page {page_number}")),
                asset_refs: vec![asset_id],
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(page_number),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::LiteParseFigure,
                metadata,
            });
        }

        let page_figure_ids: Vec<String> = ir
            .blocks
            .iter()
            .filter(|b| b.page == Some(page_number) && b.block_type == BlockType::Figure)
            .map(|b| b.block_id.clone())
            .collect();
        let page_text_ids: Vec<String> = ir
            .blocks
            .iter()
            .filter(|b| b.page == Some(page_number) && b.block_type == BlockType::Paragraph)
            .map(|b| b.block_id.clone())
            .collect();

        let fig_ids_csv = page_figure_ids.join(",");
        let txt_ids_csv = page_text_ids.join(",");

        for fig_id in &page_figure_ids {
            if let Some(fig_block) = ir.blocks.iter_mut().find(|b| b.block_id == *fig_id) {
                if !txt_ids_csv.is_empty() {
                    fig_block
                        .metadata
                        .insert("related_text_block_ids".to_string(), txt_ids_csv.clone());
                }
            }
        }
        for txt_id in &page_text_ids {
            if let Some(txt_block) = ir.blocks.iter_mut().find(|b| b.block_id == *txt_id) {
                if !fig_ids_csv.is_empty() {
                    txt_block
                        .metadata
                        .insert("related_figure_ids".to_string(), fig_ids_csv.clone());
                }
            }
        }

        if let Some(page) = ir.pages.iter_mut().find(|p| p.page_number == page_number) {
            page.image_count = images.len();
        }
    }
}

fn is_decorative_image(
    img: &ingestion::parser::ExtractedPdfImage,
    page_dims: Option<(f32, f32)>,
    decorative_max_area: f32,
) -> bool {
    let img_area = (img.width as f32) * (img.height as f32);
    if let Some((page_w, page_h)) = page_dims {
        let page_area = page_w * page_h;
        if page_area > 0.0 {
            return (img_area / page_area) <= decorative_max_area;
        }
    }
    img.width <= 50 && img.height <= 50
}

fn write_temp_figure(
    img: &ingestion::parser::ExtractedPdfImage,
    page_number: u32,
    fig_idx: usize,
) -> anyhow::Result<String> {
    use ingestion::parser::PdfImageFormat;

    let ext = match img.content_type {
        PdfImageFormat::Jpeg => "jpg",
        PdfImageFormat::Png => "png",
        PdfImageFormat::Raw => "bin",
    };
    let path = std::env::temp_dir().join(format!("avrag-bfig-p{page_number}-{fig_idx}.{ext}"));
    std::fs::write(&path, &img.data)?;
    Ok(format!("temporary://{}", path.display()))
}

async fn summarize_figure_with_vlm(
    llm: &Arc<LlmClient>,
    image_data_url: &str,
    page_number: u32,
) -> anyhow::Result<String> {
    let prompt = format!(
        "This is a figure/image extracted from page {page_number} of a PDF document. \
         Provide a concise factual summary (2-3 sentences) describing what this figure shows. \
         Focus on content visible in the image: charts, diagrams, photos, illustrations, text overlays. \
         Return the summary only, no preamble."
    );

    let messages = vec![
        ChatMessage::system(
            "You summarize document figures for a RAG index. Be factual and concise.",
        ),
        ChatMessage::user_multimodal(prompt, vec![image_data_url.to_string()]),
    ];

    let response = llm.complete(&messages, Some(0.1)).await?;
    Ok(response.content)
}

#[cfg(test)]
mod tests {
    use ingestion::parser::{
        PageRouteKind, PdfImageFormat, PdfPageBackend, PdfPagePlan, PdfParsePlan, RouteReason,
    };
    use ingestion::{DocumentIr, DocumentType, ParseBackend};

    use super::*;

    #[tokio::test]
    async fn figure_block_created_without_vlm() {
        let ctx = PdfParseContext {
            ingestion_llm: None,
            pdf_renderer_client: None,
        };
        let plan = PdfParsePlan {
            pages: vec![PdfPagePlan {
                page_number: 1,
                backend: PdfPageBackend::EdgeParse,
                reason: RouteReason::FastWithFigures,
                route_kinds: vec![PageRouteKind::Figure],
            }],
        };
        let mut ir = DocumentIr::new(
            "doc".to_string(),
            "test.pdf".to_string(),
            DocumentType::Pdf,
            ParseBackend::LiteParsePdf,
        );
        ir.pages.push(ingestion::PageIr {
            page_number: 1,
            backend: ParseBackend::LiteParsePdf,
            width: Some(612.0),
            height: Some(792.0),
            ..Default::default()
        });

        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/spike/fixtures/phase0-mini.pdf");
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(path).expect("read fixture");

        enrich_b_class_figures(&ctx, &bytes, &mut ir, &plan).await;

        let figure_blocks: Vec<_> = ir
            .blocks
            .iter()
            .filter(|b| b.block_type == BlockType::Figure)
            .collect();
        if figure_blocks.is_empty() {
            // Fixture may have no extractable non-decorative images on page 1.
            return;
        }
        assert!(
            figure_blocks
                .iter()
                .any(|b| b.metadata.get("vlm_failed") == Some(&"true".to_string())),
            "figure blocks without VLM should be marked vlm_failed"
        );
    }

    #[test]
    fn write_temp_figure_uses_temporary_scheme() {
        let img = ingestion::parser::ExtractedPdfImage {
            object_id: (1, 0),
            width: 100,
            height: 80,
            content_type: PdfImageFormat::Png,
            data: vec![0x89, 0x50, 0x4e, 0x47],
        };
        let path = write_temp_figure(&img, 2, 0).expect("write temp figure");
        assert!(path.starts_with("temporary://"));
        let local = path.strip_prefix("temporary://").expect("temp prefix");
        assert!(std::path::Path::new(local).exists());
        let _ = std::fs::remove_file(local);
    }
}
