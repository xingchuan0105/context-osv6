use avrag_retrieval_data_plane::TextChunkIndexRecord;
use contracts::auth_runtime::AuthContext;
use ingestion::{DocumentIr, IngestionError, IngestionTask};
use uuid::Uuid;

use super::super::helpers::{
    GraphIndexRecords, build_document_index_batch, build_graph_index_records,
    build_text_index_records, extract_triplets_for_index, extract_visual_triplets_for_index,
    merge_extracted_triplets, triplet_extraction_enabled, visual_triplet_extraction_enabled,
};
use super::super::index_dispatch::embed_text_vectors;
use super::super::processor::PgTaskProcessor;
use crate::indexing::{StoredMultimodalChunk, build_multimodal_index_records};
use crate::ingestion_guard::ensure_ingestion_side_effects_allowed;

use super::ParseRunState;
use super::materialize::MaterializeOutput;

/// Dual-write figure VLM `context_text` into the text dense index (same `chunk_id`
/// as `document_multimodal_chunks` so citation lookup can resolve `asset_id`).
async fn build_figure_desc_text_index_records(
    processor: &PgTaskProcessor,
    mm_chunks: &[StoredMultimodalChunk],
) -> Result<Vec<TextChunkIndexRecord>, IngestionError> {
    let candidates: Vec<&StoredMultimodalChunk> = mm_chunks
        .iter()
        .filter(|c| {
            let t = c.context_text.trim();
            // Skip empty / placeholder-only raster stubs.
            t.chars().count() >= 24 && !t.starts_with("PDF page")
        })
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let texts: Vec<&str> = candidates.iter().map(|c| c.context_text.as_str()).collect();
    let vectors = embed_text_vectors(processor, &texts).await?;
    if vectors.len() != candidates.len() {
        return Err(IngestionError::embedding(format!(
            "figure-desc embedding count ({}) != chunk count ({})",
            vectors.len(),
            candidates.len()
        )));
    }
    Ok(candidates
        .into_iter()
        .zip(vectors)
        .map(|(chunk, vector)| {
            // Embed figure asset binding in source_locator so text dense hits can
            // populate Citation.asset_id / image_url without MM ANN.
            let mut locator = chunk
                .source_locator
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            if let Some(obj) = locator.as_object_mut() {
                obj.insert(
                    "asset_id".to_string(),
                    serde_json::Value::String(chunk.asset_id.to_string()),
                );
                if !chunk.image_path.trim().is_empty() {
                    obj.insert(
                        "image_path".to_string(),
                        serde_json::Value::String(chunk.image_path.clone()),
                    );
                }
                if let Some(cap) = chunk.caption.as_ref().filter(|c| !c.trim().is_empty()) {
                    obj.insert(
                        "caption".to_string(),
                        serde_json::Value::String(cap.clone()),
                    );
                }
                obj.insert(
                    "figure_desc".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
            TextChunkIndexRecord {
                chunk_id: chunk.chunk_id,
                content: chunk.context_text.clone(),
                vector,
                page: chunk.page,
                chunk_type: if chunk.chunk_type.is_empty() {
                    "figure_desc".to_string()
                } else {
                    format!("figure_desc:{}", chunk.chunk_type)
                },
                parser_backend: Some(chunk.parser_backend.clone()),
                source_locator: Some(locator),
            }
        })
        .collect())
}

pub(crate) async fn stage_build_and_replace_retrieval_index(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    workspace_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    document_ir: &DocumentIr,
    materialize: &MaterializeOutput,
    parse_run_state: &mut ParseRunState,
) -> Result<(), IngestionError> {
    let needs_text_vector_index = processor.storage.retrieval_data_plane.is_some()
        && processor.embedding.embedding_client.is_some();
    let embed_started = std::time::Instant::now();
    let mut text_index_records = if needs_text_vector_index {
        tracing::info!(
            stage = "index_embed",
            document_id = %document_id,
            chunk_count = materialize.chunks.len(),
            "ingestion index: embedding text chunks"
        );
        build_text_index_records(processor, &materialize.chunks).await?
    } else {
        Vec::new()
    };
    // 2026-08-04: dual-write VLM figure descriptions into the **text** vector space
    // so dense retrieval finds figures without MM ANN (mm-off plan).
    if needs_text_vector_index {
        let figure_text_records = build_figure_desc_text_index_records(
            processor,
            &materialize.stored_multimodal_chunks,
        )
        .await?;
        if !figure_text_records.is_empty() {
            tracing::info!(
                stage = "index_embed",
                document_id = %document_id,
                figure_desc_chunks = figure_text_records.len(),
                "ingestion index: embedding figure VLM descriptions as text"
            );
            text_index_records.extend(figure_text_records);
        }
    }
    if needs_text_vector_index {
        tracing::info!(
            stage = "index_embed",
            document_id = %document_id,
            vectors = text_index_records.len(),
            elapsed_ms = embed_started.elapsed().as_millis(),
            "ingestion index: text embedding done"
        );
    }
    if !text_index_records.is_empty() {
        parse_run_state.outputs.text_vector_count = text_index_records.len();
    }

    let needs_multimodal_vector_index = processor.storage.retrieval_data_plane.is_some();
    let multimodal_index_records = if needs_multimodal_vector_index {
        build_multimodal_index_records(
            processor,
            document_ir,
            &materialize.stored_multimodal_chunks,
            &mut parse_run_state.outputs,
        )
        .await?
    } else {
        Vec::new()
    };
    if !multimodal_index_records.is_empty() {
        parse_run_state.outputs.multimodal_vector_count = multimodal_index_records.len();
    }

    let graph_records = if processor.storage.retrieval_data_plane.is_some()
        && triplet_extraction_enabled()
    {
        let mut extraction = extract_triplets_for_index(
            processor,
            document_id,
            &text_index_records,
            parse_run_state,
        )
        .await;
        if visual_triplet_extraction_enabled() {
            let visual = extract_visual_triplets_for_index(
                processor,
                document_id,
                &materialize.stored_multimodal_chunks,
                parse_run_state,
            )
            .await;
            extraction.total_tokens = extraction.total_tokens.saturating_add(visual.total_tokens);
            extraction.triplets = merge_extracted_triplets(extraction.triplets, visual.triplets);
        }
        if extraction.total_tokens > 0 {
            let _ = processor
                .storage
                .repo
                .sessions()
                .record_usage_event(
                    context,
                    "triplet_extraction_tokens",
                    i64::from(extraction.total_tokens),
                    "worker_ingestion",
                )
                .await;
        }
        build_graph_index_records(processor, extraction.triplets, parse_run_state).await
    } else {
        GraphIndexRecords::default()
    };

    if let Some(data_plane) = &processor.storage.retrieval_data_plane {
        ensure_ingestion_side_effects_allowed(
            &processor.storage.repo,
            context,
            task,
            document_id,
            "retrieval index replace",
        )
        .await?;
        let batch = build_document_index_batch(
            context,
            Some(workspace_id),
            document_id,
            parse_run_id,
            text_index_records,
            multimodal_index_records,
            graph_records,
        );
        let report = data_plane
            .replace_document_index(batch)
            .await
            .map_err(|error| {
                IngestionError::index(format!("retrieval data plane indexing failed: {error}"))
            })?;
        parse_run_state.outputs.text_vector_count = report.text_chunk_count;
        parse_run_state.outputs.multimodal_vector_count = report.multimodal_chunk_count;
        parse_run_state.outputs.entity_count = report.entity_count;
        parse_run_state.outputs.relation_count = report.relation_count;
        parse_run_state.outputs.graph_passage_count = report.graph_passage_count;
    }

    Ok(())
}
