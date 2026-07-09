use avrag_llm::ChatMessage;
use tracing::warn;

use super::env::{env_flag_enabled, vlm_summary_enabled};
use super::media::{MediaResolveContext, resolve_visual_chunk_image_refs};
use super::types::{StoredMultimodalChunk, record_multimodal_degrade};

pub async fn maybe_enrich_visual_multimodal_summaries(
    processor: &crate::PgTaskProcessor,
    chunks: &mut [StoredMultimodalChunk],
    outputs: &mut crate::ParseRunOutputs,
) {
    if !vlm_summary_enabled() {
        return;
    }
    let Some(llm) = processor.llm.ingestion_llm.clone() else {
        return;
    };
    let skip_raster_for_ocr = !env_flag_enabled("INGESTION_PAGE_RASTER_WITH_OCR", false);
    let media_ctx = MediaResolveContext {
        object_store: processor.storage.object_store.clone(),
        asset_url_ttl_secs: processor.storage.asset_url_ttl_secs,
    };

    for chunk in chunks.iter_mut() {
        if chunk.chunk_type != "page_raster" {
            continue;
        }
        // ING-4: When OCR was used, page_raster chunks are unlikely to be useful.
        // Architecturally, PaddleOCR pages don't produce PageRaster blocks.
        // This gate is for edge cases where VisualRaster fallback created rasters for OCR-failed pages.
        if skip_raster_for_ocr && chunk.parser_backend == "visual_raster_pdf" {
            if chunk.context_text.is_empty() || chunk.context_text.starts_with("PDF page") {
                continue;
            }
        }
        let image_refs = match resolve_visual_chunk_image_refs(&media_ctx, chunk).await {
            Ok(refs) => refs,
            Err(error) => {
                record_multimodal_degrade(
                    outputs,
                    format!(
                        "chunk {}: failed to resolve page images for VLM summary: {error}",
                        chunk.chunk_id
                    ),
                );
                continue;
            }
        };
        if image_refs.is_empty() {
            record_multimodal_degrade(
                outputs,
                format!(
                    "chunk {}: VLM summary skipped because no resolvable page images were found",
                    chunk.chunk_id
                ),
            );
            continue;
        }
        let caption = chunk
            .caption
            .clone()
            .unwrap_or_else(|| "PDF page raster".to_string());
        let image_list = image_refs.join(", ");
        let prompt = format!(
            "Summarize the readable content of these PDF page images for retrieval. \
             Caption: {caption}. Image URL(s): {image_list}. \
             Return 2-4 sentences of factual summary only."
        );
        let messages = vec![
            ChatMessage::system(
                "You summarize document page images for a RAG index. Be factual and concise.",
            ),
            ChatMessage::user(prompt),
        ];
        match llm.complete(&messages, Some(0.1)).await {
            Ok(response) if !response.content.trim().is_empty() => {
                chunk.context_text = response.content.trim().to_string();
            }
            Ok(_) => {
                record_multimodal_degrade(
                    outputs,
                    format!(
                        "chunk {}: VLM summary returned empty content",
                        chunk.chunk_id
                    ),
                );
            }
            Err(error) => {
                record_multimodal_degrade(
                    outputs,
                    format!("chunk {}: VLM summary failed: {error}", chunk.chunk_id),
                );
                warn!(
                    chunk_id = %chunk.chunk_id,
                    error = %error,
                    "visual multimodal VLM summary failed"
                );
            }
        }
    }
}
