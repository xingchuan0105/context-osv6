use avrag_llm::ChatMessage;
use tracing::{info, warn};

use super::env::{env_flag_enabled, vlm_summary_enabled};
use super::media::{MediaResolveContext, resolve_visual_chunk_image_refs};
use super::types::{StoredMultimodalChunk, record_multimodal_degrade};

/// Chunk types that represent figures / page visuals worth VLM description.
/// OCR-success full-page rasters are gated separately (ING-4).
fn is_visual_desc_chunk(chunk_type: &str) -> bool {
    matches!(
        chunk_type,
        "page_raster"
            | "figure"
            | "image"
            | "picture"
            | "chart"
            | "diagram"
            | "photo"
            | "screenshot"
    ) || chunk_type.contains("figure")
        || chunk_type.contains("image")
}

/// Retrieval-oriented figure/page description via ingestion LLM (true multimodal).
///
/// Plan: `docs/engineering/2026-08-04-mm-off-vlm-figure-text-plan.md`
/// - Write description into `context_text` for text-index dual-write + citation.
/// - Does **not** require MM embedding.
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

    let mut enriched = 0usize;
    for chunk in chunks.iter_mut() {
        if !is_visual_desc_chunk(&chunk.chunk_type) {
            continue;
        }
        // ING-4: When OCR was used, page_raster chunks are unlikely to be useful.
        // Architecturally, PaddleOCR pages don't produce PageRaster blocks.
        // This gate is for edge cases where VisualRaster fallback created rasters for OCR-failed pages.
        if skip_raster_for_ocr && chunk.chunk_type == "page_raster" {
            if chunk.parser_backend == "visual_raster_pdf"
                && (chunk.context_text.is_empty() || chunk.context_text.starts_with("PDF page"))
            {
                continue;
            }
            // If context already looks like OCR body text, skip VLM.
            if chunk.context_text.chars().count() > 200 {
                continue;
            }
        }

        let image_refs = match resolve_visual_chunk_image_refs(&media_ctx, chunk).await {
            Ok(refs) => refs,
            Err(error) => {
                record_multimodal_degrade(
                    outputs,
                    format!(
                        "chunk {}: failed to resolve images for VLM summary: {error}",
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
                    "chunk {}: VLM summary skipped because no resolvable images were found",
                    chunk.chunk_id
                ),
            );
            continue;
        }

        let caption = chunk
            .caption
            .clone()
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| chunk.chunk_type.clone());
        let prompt = format!(
            "Describe this document figure/image for retrieval indexing.\n\
             Caption/type: {caption}.\n\
             Write 2-6 factual sentences in the document's language covering: \
             what the figure shows, labeled entities, relationships/arrows, and any \
             readable numbers or key text. No markdown, no preamble."
        );
        // True multimodal: model must receive image parts (not URL-only text).
        let messages = vec![
            ChatMessage::system(
                "You write short retrieval descriptions of document figures. Be factual and concise.",
            ),
            ChatMessage::user_multimodal(prompt, image_refs),
        ];
        match llm.complete(&messages, Some(0.1)).await {
            Ok(response) if !response.content.trim().is_empty() => {
                let desc = response.content.trim().to_string();
                chunk.context_text = desc;
                enriched += 1;
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
    if enriched > 0 {
        info!(enriched, "VLM figure descriptions written to context_text");
    }
}
