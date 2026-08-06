use crate::indexing::{
    MediaResolveContext, StoredMultimodalChunk, env_flag_enabled, resolve_visual_chunk_image_refs,
};
use anyhow::Result;
use avrag_llm::ChatMessage;
use avrag_retrieval_data_plane::TextChunkIndexRecord;
use tracing::info;
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::helpers::{record_graph_degrade, TRIPLET_TEMPERATURE};
use super::processor::PgTaskProcessor;
use super::triplet_semantic_lint::triplet_semantic_violation;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtractedTriplet {
    pub(crate) subject: String,
    pub(crate) predicate: String,
    pub(crate) object: String,
    pub(crate) supporting_chunk_ids: Vec<Uuid>,
    pub(crate) source: String,
    pub(crate) confidence: f32,
}

#[derive(Debug, Default, Clone)]
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
    let Some(llm) = processor.llm.ingestion_llm.clone() else {
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
        object_store: processor.storage.object_store.clone(),
        asset_url_ttl_secs: processor.storage.asset_url_ttl_secs,
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
        match complete_triplet_extraction(
            &llm,
            processor.llm.completion_cache.as_ref(),
            &messages,
        )
        .await {
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
    _processor: &PgTaskProcessor,
    document_id: Uuid,
    _text_chunks: &[TextChunkIndexRecord],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    if let Some(pending) = parse_run_state.pending_triplets.take() {
        info!(
            document_id = %document_id,
            triplets = pending.triplets.len(),
            "using windowed-session triplets"
        );
        return pending;
    }
    info!(
        document_id = %document_id,
        "no pending windowed triplets; graph extraction skipped"
    );
    TripletExtractionOutput::default()
}

const TRIPLET_EXTRACTION_SYSTEM_PROMPT: &str =
    include_str!("../../../../prompts/pipeline/triplet-extraction.system.md");

/// DeepSeek v4-flash non-reasoning (`thinking: disabled`) may wrap JSON in markdown
/// fences. Strip them in `parse_triplet_response`; do not enable JSON Output mode here
/// (adds latency and can exceed ingest timeouts on large documents).
async fn complete_triplet_extraction(
    llm: &avrag_llm::LlmClient,
    cache: Option<&avrag_llm::CompletionCache>,
    messages: &[ChatMessage],
) -> Result<avrag_llm::LlmResponse> {
    let model = llm.config.model.clone();
    if let Some(cache) = cache {
        if let Some(hit) = cache
            .get(&model, TRIPLET_EXTRACTION_SYSTEM_PROMPT, messages)
            .await
        {
            return Ok(avrag_llm::LlmResponse {
                content: hit.content,
                reasoning_content: hit.reasoning_content,
                usage: avrag_llm::LlmUsage::zeroed(),
                model,
                tool_calls: None,
                response_id: None,
            });
        }
    }
    // Large batches can emit long JSON arrays; cap high enough to avoid truncation.
    let response = llm
        .complete_with_max_tokens(messages, Some(TRIPLET_TEMPERATURE), 8_192)
        .await?;
    if let Some(cache) = cache {
        cache
            .store(
                &model,
                TRIPLET_EXTRACTION_SYSTEM_PROMPT,
                messages,
                &avrag_llm::CachedCompletion {
                    content: response.content.clone(),
                    reasoning_content: response.reasoning_content.clone(),
                },
            )
            .await;
    }
    Ok(response)
}

fn normalize_triplet_json_payload(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut body = String::new();
    for line in trimmed.lines().skip(1) {
        if line.trim() == "```" {
            break;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(line);
    }
    body.trim().to_string()
}

pub(crate) fn parse_triplet_response(
    content: &str,
    valid_chunk_ids: &[Uuid],
) -> Result<Vec<ExtractedTriplet>> {
    let normalized = normalize_triplet_json_payload(content);
    match serde_json::from_str::<serde_json::Value>(&normalized) {
        Ok(value) => Ok(collect_triplets_from_value(&value, Some(valid_chunk_ids))),
        Err(primary_error) => {
            let salvaged = salvage_triplet_objects(&normalized, Some(valid_chunk_ids));
            if salvaged.is_empty() {
                return Err(anyhow::anyhow!(
                    "Failed to parse triplet JSON: {primary_error}"
                ));
            }
            Ok(salvaged)
        }
    }
}

/// Windowed text path: no chunk_id required.
pub(crate) fn parse_triplet_response_no_chunk(content: &str) -> Result<Vec<ExtractedTriplet>> {
    let normalized = normalize_triplet_json_payload(content);
    match serde_json::from_str::<serde_json::Value>(&normalized) {
        Ok(value) => Ok(collect_triplets_from_value(&value, None)),
        Err(primary_error) => {
            let salvaged = salvage_triplet_objects(&normalized, None);
            if salvaged.is_empty() {
                return Err(anyhow::anyhow!(
                    "Failed to parse triplet JSON: {primary_error}"
                ));
            }
            Ok(salvaged)
        }
    }
}

fn collect_triplets_from_value(
    value: &serde_json::Value,
    valid_chunk_ids: Option<&[Uuid]>,
) -> Vec<ExtractedTriplet> {
    let Some(triplets) = value.get("triplets").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };

    let mut parsed = Vec::new();
    for item in triplets {
        if let Some(triplet) = parse_triplet_item(item, valid_chunk_ids) {
            parsed.push(triplet);
        }
    }
    parsed
}

fn parse_triplet_item(
    item: &serde_json::Value,
    valid_chunk_ids: Option<&[Uuid]>,
) -> Option<ExtractedTriplet> {
    let supporting_chunk_ids = if let Some(valid) = valid_chunk_ids {
        let chunk_id_str = item.get("chunk_id")?.as_str()?;
        let chunk_id = Uuid::parse_str(chunk_id_str).ok()?;
        if !valid.contains(&chunk_id) {
            return None;
        }
        vec![chunk_id]
    } else {
        item.get("chunk_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(|id| vec![id])
            .unwrap_or_default()
    };

    let subject = item
        .get("subject")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let predicate_raw = item
        .get("predicate")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let (predicate, _pred_orig) =
        super::predicate_normalize::normalize_predicate(&predicate_raw);
    if predicate.is_empty() {
        return None;
    }
    let object = item
        .get("object")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    if triplet_semantic_violation(&subject, &predicate, &object).is_some() {
        return None;
    }

    let confidence = item
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(1.0);
    let source = item
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(if valid_chunk_ids.is_some() {
            "text_chunk"
        } else {
            "window_text"
        })
        .to_string();

    Some(ExtractedTriplet {
        subject,
        predicate,
        object,
        supporting_chunk_ids,
        source,
        confidence,
    })
}

/// Recover complete triplet objects when the model response is truncated mid-JSON.
fn salvage_triplet_objects(
    normalized: &str,
    valid_chunk_ids: Option<&[Uuid]>,
) -> Vec<ExtractedTriplet> {
    let mut salvaged = Vec::new();
    let markers = ["{\"chunk_id\"", "{\"subject\""];
    let mut search_from = 0;
    while search_from < normalized.len() {
        let mut next_rel = None;
        for m in markers {
            if let Some(rel) = normalized[search_from..].find(m) {
                let abs = search_from + rel;
                next_rel = Some(match next_rel {
                    Some(prev) if prev < abs => prev,
                    _ => abs,
                });
            }
        }
        let Some(start) = next_rel else {
            break;
        };
        let mut parsed_any = false;
        let mut end = (start + 20).min(normalized.len());
        while end <= normalized.len() {
            if normalized.is_char_boundary(end) {
                if let Ok(item) = serde_json::from_str::<serde_json::Value>(&normalized[start..end])
                    && item.get("subject").is_some()
                    && item.get("predicate").is_some()
                    && item.get("object").is_some()
                    && let Some(triplet) = parse_triplet_item(&item, valid_chunk_ids)
                {
                    salvaged.push(triplet);
                    search_from = end;
                    parsed_any = true;
                    break;
                }
            }
            if end == normalized.len() {
                break;
            }
            end += 1;
        }
        if !parsed_any {
            search_from = start + 1;
        }
    }
    salvaged
}

pub(crate) fn triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_TRIPLET_ENABLED", true)
}

pub(crate) fn visual_triplet_extraction_enabled() -> bool {
    env_flag_enabled("INGESTION_VLM_TRIPLET_ENABLED", false)
}
