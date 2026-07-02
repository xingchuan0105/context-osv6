use avrag_retrieval_data_plane::TextChunkIndexRecord;
use avrag_storage_pg;
use ingestion::IngestionError;
use uuid::Uuid;

use super::processor::PgTaskProcessor;

pub(crate) async fn build_text_index_records(
    processor: &PgTaskProcessor,
    chunks: &[avrag_storage_pg::IndexedChunk],
) -> Result<Vec<TextChunkIndexRecord>, IngestionError> {
    let texts = chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<Vec<_>>();
    let vectors = embed_text_vectors(processor, &texts).await?;

    if vectors.len() != chunks.len() {
        return Err(IngestionError::StateSink(format!(
            "embedding count ({}) != chunk count ({}) — refusing to silently drop chunks",
            vectors.len(),
            chunks.len()
        )));
    }

    chunks
        .iter()
        .zip(vectors)
        .map(|(chunk, vector)| {
            let chunk_id = Uuid::parse_str(&chunk.chunk_id)
                .map_err(|error| IngestionError::StateSink(format!("invalid chunk id: {error}")))?;
            Ok(TextChunkIndexRecord {
                chunk_id,
                content: chunk.content.clone(),
                vector,
                page: chunk.page,
                chunk_type: metadata_string(&chunk.metadata, "block_type")
                    .or_else(|| metadata_string(&chunk.metadata, "kind"))
                    .unwrap_or_else(|| "body".to_string()),
                parser_backend: metadata_string(&chunk.metadata, "parser_backend"),
                source_locator: metadata_value(&chunk.metadata, "source_locator"),
            })
        })
        .collect()
}

pub(crate) async fn embed_text_vectors(
    processor: &PgTaskProcessor,
    texts: &[&str],
) -> Result<Vec<Vec<f32>>, IngestionError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if processor.embedding_client.is_none() {
        return Err(IngestionError::StateSink(format!(
            "text embedding client not configured (expected dim {})",
            processor.embedding_dim
        )));
    }
    embed_text_vectors_with_client(processor.embedding_client.as_ref(), texts).await
}

pub(crate) async fn embed_text_vectors_with_client(
    client: Option<&avrag_llm::EmbeddingClient>,
    texts: &[&str],
) -> Result<Vec<Vec<f32>>, IngestionError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let Some(client) = client else {
        return Err(IngestionError::StateSink(
            "text embedding client not configured".to_string(),
        ));
    };
    client
        .embed(texts)
        .await
        .map_err(|error| IngestionError::StateSink(format!("embedding failed: {error}")))
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn metadata_value(metadata: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    metadata.get(key).cloned().filter(|value| !value.is_null())
}
