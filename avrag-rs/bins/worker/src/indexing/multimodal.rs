use std::sync::Arc;

use avrag_retrieval_data_plane::{MultimodalChunkIndexRecord, multimodal_retrieval_weight};
use ingestion::{DocumentIr, IngestionError, parse_page_status_from_ir};
use tracing::{info, warn};
use uuid::Uuid;

use super::media::{build_multimodal_embed_input, MediaResolveContext};
use super::page_status::document_paddle_ocr_succeeded;
use super::env::env_flag_enabled;
use super::types::{record_multimodal_degrade, StoredMultimodalChunk};

pub async fn build_multimodal_index_records(
    processor: &crate::PgTaskProcessor,
    document_ir: &DocumentIr,
    chunks: &[StoredMultimodalChunk],
    outputs: &mut crate::ParseRunOutputs,
) -> Result<Vec<MultimodalChunkIndexRecord>, IngestionError> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let page_status = parse_page_status_from_ir(document_ir);

    let Some(client) = processor.mm_embedding_client.as_ref() else {
        info!(
            multimodal_chunks = chunks.len(),
            "MM embedding not configured; skipping multimodal vector indexing"
        );
        return Ok(Vec::new());
    };

    let media_ctx = MediaResolveContext {
        object_store: processor.object_store.clone(),
        asset_url_ttl_secs: processor.asset_url_ttl_secs,
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let client = client.clone();
    type MmEmbeddingHandle =
        tokio::task::JoinHandle<anyhow::Result<Option<(usize, Vec<f32>)>>>;
    let mut handles: Vec<(usize, Uuid, MmEmbeddingHandle)> = Vec::with_capacity(chunks.len());

    let paddle_ocr_succeeded = document_paddle_ocr_succeeded(document_ir);
    let skip_page_raster_ocr = !env_flag_enabled("INGESTION_PAGE_RASTER_WITH_OCR", false);

    for (idx, chunk) in chunks.iter().enumerate() {
        if skip_page_raster_ocr && chunk.chunk_type == "page_raster" && paddle_ocr_succeeded {
            continue;
        }
        let caption = chunk
            .caption
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| chunk.context_text.clone());
        let chunk_id = chunk.chunk_id;
        let had_fusion = chunk.fusion_image_paths.len() > 1;
        let sem = semaphore.clone();
        let c = client.clone();
        let ctx = media_ctx.clone();
        let chunk = chunk.clone();
        handles.push((
            idx,
            chunk_id,
            tokio::spawn(async move {
                let _permit = sem
                    .acquire_owned()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                let input = build_multimodal_embed_input(&ctx, &chunk, caption).await;
                let used_text_only = input.image.is_none()
                    && input.images.is_empty()
                    && input.text.is_some();
                if used_text_only {
                    anyhow::bail!(
                        "chunk {chunk_id} has no resolvable image{}",
                        if had_fusion {
                            " for fusion embed"
                        } else {
                            ""
                        }
                    );
                }
                let vector = c.embed_multimodal_fused(&input, None).await?;
                Ok(Some((idx, vector)))
            }),
        ));
    }

    let mut indexed_embeddings: Vec<(usize, Vec<f32>)> = Vec::new();
    for (idx, chunk_id, handle) in handles {
        match handle.await {
            Ok(Ok(Some((embedding_idx, vector)))) => {
                indexed_embeddings.push((embedding_idx, vector));
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                record_multimodal_degrade(
                    outputs,
                    format!("chunk {chunk_id}: multimodal embed failed: {error}"),
                );
                warn!(
                    chunk_id = %chunk_id,
                    chunk_index = idx,
                    error = %error,
                    "multimodal chunk embed failed; skipping vector index"
                );
            }
            Err(error) => {
                record_multimodal_degrade(
                    outputs,
                    format!("chunk {chunk_id}: multimodal embed task join failed: {error}"),
                );
            }
        }
    }
    indexed_embeddings.sort_by_key(|(idx, _)| *idx);

    Ok(indexed_embeddings
        .into_iter()
        .map(|(idx, vector)| {
            let chunk = &chunks[idx];
            let page_ocr_failed = chunk
                .page
                .and_then(|p| page_status.get(&(p as u32)))
                .map(|status| status.is_ocr_fail())
                .unwrap_or(false);
            let retrieval_weight = multimodal_retrieval_weight(&chunk.chunk_type, page_ocr_failed);
            MultimodalChunkIndexRecord {
                chunk_id: chunk.chunk_id,
                asset_id: chunk.asset_id,
                context_text: chunk.context_text.clone(),
                caption: chunk.caption.clone(),
                image_path: Some(chunk.image_path.clone()),
                vector,
                page: chunk.page,
                chunk_type: chunk.chunk_type.clone(),
                parser_backend: Some(chunk.parser_backend.clone()),
                source_locator: chunk.source_locator.clone(),
                retrieval_weight,
            }
        })
        .collect())
}
