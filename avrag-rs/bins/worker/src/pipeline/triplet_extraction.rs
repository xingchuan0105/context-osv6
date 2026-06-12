use anyhow::Result;
use avrag_llm::ChatMessage;
use avrag_retrieval_data_plane::TextChunkIndexRecord;
use crate::indexing::{
    env_flag_enabled, resolve_visual_chunk_image_refs, MediaResolveContext, StoredMultimodalChunk,
};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::processor::PgTaskProcessor;
use super::helpers::{estimate_token_count, record_graph_degrade};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtractedTriplet {
    pub(crate) subject: String,
    pub(crate) predicate: String,
    pub(crate) object: String,
    pub(crate) supporting_chunk_ids: Vec<Uuid>,
    pub(crate) source: String,
    pub(crate) confidence: f32,
}

#[derive(Debug, Clone)]
struct TripletExtractionBatch {
    chunk_ids: Vec<Uuid>,
    payload: serde_json::Value,
}

#[derive(Debug, Default)]
pub(crate) struct TripletExtractionOutput {
    pub(crate) triplets: Vec<ExtractedTriplet>,
    pub(crate) total_tokens: u32,
}

pub(crate) fn merge_extracted_triplets(
    mut base: Vec<ExtractedTriplet>,
    extra: Vec<ExtractedTriplet>,
) -> Vec<ExtractedTriplet> {
    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();
    for triplet in base.drain(..).chain(extra) {
        let key = (
            triplet.subject.to_lowercase(),
            triplet.predicate.to_lowercase(),
            triplet.object.to_lowercase(),
        );
        if let Some(existing) = triplet_map.get_mut(&key) {
            for chunk_id in triplet.supporting_chunk_ids {
                if !existing.supporting_chunk_ids.contains(&chunk_id) {
                    existing.supporting_chunk_ids.push(chunk_id);
                }
            }
            if triplet.confidence > existing.confidence {
                existing.confidence = triplet.confidence;
            }
        } else {
            triplet_map.insert(key, triplet);
        }
    }
    triplet_map.into_values().collect()
}

pub(crate) async fn extract_visual_triplets_for_index(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    multimodal_chunks: &[StoredMultimodalChunk],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    let Some(llm) = processor.triplet_llm.clone() else {
        return TripletExtractionOutput::default();
    };

    let visual_chunks: Vec<&StoredMultimodalChunk> = multimodal_chunks
        .iter()
        .filter(|chunk| chunk.chunk_type == "page_raster")
        .collect();
    if visual_chunks.is_empty() {
        return TripletExtractionOutput::default();
    }

    let media_ctx = MediaResolveContext {
        object_store: processor.object_store.clone(),
        asset_url_ttl_secs: processor.asset_url_ttl_secs,
    };
    let mut output = TripletExtractionOutput::default();
    for chunk in visual_chunks {
        let image_refs = match resolve_visual_chunk_image_refs(&media_ctx, chunk).await {
            Ok(refs) => refs,
            Err(error) => {
                record_graph_degrade(
                    &mut parse_run_state.outputs,
                    format!(
                        "chunk {}: visual triplet skipped (image resolve failed): {error}",
                        chunk.chunk_id
                    ),
                );
                continue;
            }
        };
        if image_refs.is_empty() {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!(
                    "chunk {}: visual triplet skipped (no resolvable page images)",
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
            "Extract up to 3 factual subject-predicate-object triplets from this page image. \
             Return JSON: {{\"triplets\":[{{\"chunk_id\":\"{}\",\"subject\":\"...\",\"predicate\":\"...\",\"object\":\"...\",\"confidence\":0.0-1.0,\"source\":\"vlm_page_summary\"}}]}}. \
             Caption: {caption}. Image URL(s): {image_list}",
            chunk.chunk_id
        );
        let messages = vec![
            ChatMessage::system(
                "Extract knowledge-graph triplets from document page images. JSON only.",
            ),
            ChatMessage::user(prompt),
        ];
        match llm.complete(&messages, Some(0.1)).await {
            Ok(response) => {
                output.total_tokens = output
                    .total_tokens
                    .saturating_add(response.usage.total_tokens);
                match parse_triplet_response(&response.content, &[chunk.chunk_id]) {
                    Ok(mut triplets) => {
                        for triplet in &mut triplets {
                            if triplet.source.is_empty() {
                                triplet.source = "vlm_page_summary".to_string();
                            }
                        }
                        output.triplets.extend(triplets);
                    }
                    Err(error) => {
                        let reason = format!("visual triplet extraction failed: {error}");
                        record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                        info!(document_id = %document_id, error = %reason, "visual triplet extraction degraded");
                    }
                }
            }
            Err(error) => {
                let reason = format!("visual triplet extraction failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "visual triplet extraction degraded");
            }
        }
    }
    output
}

pub(crate) async fn extract_triplets_for_index(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    text_chunks: &[TextChunkIndexRecord],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    let Some(llm) = processor.triplet_llm.clone() else {
        return TripletExtractionOutput::default();
    };

    let batches = build_triplet_extraction_batches(text_chunks);
    if batches.is_empty() {
        return TripletExtractionOutput::default();
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::with_capacity(batches.len());
    for batch in batches {
        let llm = llm.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let messages = build_triplet_extraction_messages(&batch);
            let response = llm.complete(&messages, Some(0.1)).await?;
            let raw_triplets = parse_triplet_response(&response.content, &batch.chunk_ids)?;
            Ok::<_, anyhow::Error>((raw_triplets, response.usage.total_tokens))
        }));
    }

    let mut output = TripletExtractionOutput::default();
    let mut triplet_map: std::collections::HashMap<(String, String, String), ExtractedTriplet> =
        std::collections::HashMap::new();
    for handle in handles {
        match handle.await {
            Ok(Ok((triplets, total_tokens))) => {
                output.total_tokens = output.total_tokens.saturating_add(total_tokens);
                for triplet in triplets {
                    let key = (
                        triplet.subject.to_lowercase(),
                        triplet.predicate.to_lowercase(),
                        triplet.object.to_lowercase(),
                    );
                    if let Some(existing) = triplet_map.get_mut(&key) {
                        for cid in triplet.supporting_chunk_ids {
                            if !existing.supporting_chunk_ids.contains(&cid) {
                                existing.supporting_chunk_ids.push(cid);
                            }
                        }
                    } else {
                        triplet_map.insert(key, triplet);
                    }
                }
            }
            Ok(Err(error)) => {
                let reason = format!("triplet extraction failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
            Err(error) => {
                let reason = format!("triplet extraction task join failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
        }
    }
    output.triplets = triplet_map.into_values().collect();

    output
}

fn build_triplet_extraction_batches(
    text_chunks: &[TextChunkIndexRecord],
) -> Vec<TripletExtractionBatch> {
    const TOKEN_BUDGET: i64 = 3_000;

    let mut batches = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_chunks = Vec::new();
    let mut current_tokens = 0i64;

    for chunk in text_chunks {
        let chunk_tokens = estimate_token_count(&chunk.content).max(1);
        if !current_chunks.is_empty() && current_tokens + chunk_tokens > TOKEN_BUDGET {
            batches.push(TripletExtractionBatch {
                chunk_ids: std::mem::take(&mut current_ids),
                payload: serde_json::json!({ "chunks": std::mem::take(&mut current_chunks) }),
            });
            current_tokens = 0;
        }

        current_ids.push(chunk.chunk_id);
        current_chunks.push(serde_json::json!({
            "chunk_id": chunk.chunk_id.to_string(),
            "text": &chunk.content,
        }));
        current_tokens += chunk_tokens;
    }

    if !current_chunks.is_empty() {
        batches.push(TripletExtractionBatch {
            chunk_ids: current_ids,
            payload: serde_json::json!({ "chunks": current_chunks }),
        });
    }

    batches
}

const TRIPLET_EXTRACTION_SYSTEM_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/triplet-extraction.system.md");

fn build_triplet_extraction_messages(batch: &TripletExtractionBatch) -> Vec<ChatMessage> {
    let valid_chunk_ids: Vec<String> = batch.chunk_ids.iter().map(|id| id.to_string()).collect();
    vec![
        ChatMessage::system(TRIPLET_EXTRACTION_SYSTEM_PROMPT),
        ChatMessage::user(format!(
            "Valid chunk IDs: {}\n\nChunks:\n{}\n\nExtract triplets with chunk_id:",
            valid_chunk_ids.join(", "),
            batch.payload
        )),
    ]
}

pub(crate) fn parse_triplet_response(
    content: &str,
    valid_chunk_ids: &[Uuid],
) -> Result<Vec<ExtractedTriplet>> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse triplet JSON: {}", e))?;

    let triplets = value
        .get("triplets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("triplet response missing triplets array"))?;

    let mut parsed = Vec::new();
    for item in triplets {
        // 严格对象格式：{"chunk_id": "...", "subject": "...", "predicate": "...", "object": "..."}
        let Some(chunk_id_str) = item.get("chunk_id").and_then(|v| v.as_str()) else {
            continue; // chunk_id 缺失，丢弃
        };
        let Ok(chunk_id) = Uuid::parse_str(chunk_id_str) else {
            continue; // chunk_id 无法解析，丢弃
        };
        if !valid_chunk_ids.contains(&chunk_id) {
            continue; // chunk_id 不在当前 batch 内，丢弃
        }

        let Some(subject) = item
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(predicate) = item
            .get("predicate")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(object) = item
            .get("object")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };

        let confidence = item
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(1.0);
        let source = item
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("text_chunk")
            .to_string();

        parsed.push(ExtractedTriplet {
            subject,
            predicate,
            object,
            supporting_chunk_ids: vec![chunk_id],
            source,
            confidence,
        });
    }
    Ok(parsed)
}

pub(crate) fn triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_TRIPLET_ENABLED", true)
}

pub(crate) fn visual_triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_VLM_TRIPLET_ENABLED", false)
}
