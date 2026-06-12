use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmClient};
use ingestion::parser::{PdfParsePlan, RouteReason};
use ingestion::{
    AssetIr, BlockIr, BlockModality, BlockType, DocumentIr, ParseBackend, SourceLocator,
};

use super::context::PdfParseContext;

pub async fn enrich_b_class_figures(
    ctx: &PdfParseContext,
    pdf_bytes: &[u8],
    ir: &mut DocumentIr,
    plan: &PdfParsePlan,
) {
    use ingestion::parser::{extract_page_images, image_mime_type, image_to_base64};

    let b_pages: Vec<u32> = plan
        .pages
        .iter()
        .filter(|p| p.reason == RouteReason::FastWithFigures)
        .map(|p| p.page_number)
        .collect();

    if b_pages.is_empty() {
        return;
    }

    let llm = ctx.ingestion_llm.clone();

    for page_number in b_pages {
        let images = match extract_page_images(pdf_bytes, page_number) {
            Ok(imgs) => imgs,
            Err(e) => {
                tracing::warn!(page = page_number, error = %e, "failed to extract page images");
                continue;
            }
        };

        for (fig_idx, img) in images.iter().enumerate() {
            let asset_id = format!("bfig-p{}-{}", page_number, fig_idx);
            let base64_data = image_to_base64(img);
            let mime = image_mime_type(img);
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
                storage_path: data_url,
                mime_type: Some(mime.to_string()),
                width: Some(img.width as u32),
                height: Some(img.height as u32),
                parser_backend: ParseBackend::EdgeParsePdf,
                metadata: std::collections::BTreeMap::new(),
            });

            let mut metadata = std::collections::BTreeMap::new();
            if vlm_failed {
                metadata.insert("vlm_failed".to_string(), "true".to_string());
            } else {
                metadata.insert("vlm_summarized".to_string(), "true".to_string());
            }

            if figure_text.is_empty() {
                continue;
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
                parser_backend: ParseBackend::EdgeParsePdf,
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
