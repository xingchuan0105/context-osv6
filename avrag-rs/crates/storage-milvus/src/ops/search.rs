use crate::lib_impl::MilvusDataPlane;
use crate::schema::{MULTIMODAL_OUTPUT_FIELDS, TEXT_OUTPUT_FIELDS, doc_filter};
use crate::types::Result;
use crate::utils::{optional_uuid_field, score_field, string_field, uuid_field};
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, MultimodalSearchRequest, ScoredChunk,
    TextDenseSearchRequest,
};
use serde_json::{Value, json};

impl MilvusDataPlane {
    pub(crate) async fn search_entities(
        &self,
        collection: &str,
        vector_field: &str,
        data: Value,
        filter: String,
        limit: usize,
        output_fields: &[&str],
    ) -> Result<Vec<Value>> {
        let body = self.with_database(json!({
            "collectionName": collection,
            "annsField": vector_field,
            "data": data,
            "filter": filter,
            "limit": limit,
            "outputFields": output_fields
        }));
        let response = self.post_json("/v2/vectordb/entities/search", body).await?;
        let rows = response["data"].as_array().cloned().unwrap_or_default();
        // Milvus v2.6+ nests output fields under "entity"; flatten for compatibility.
        let flattened: Vec<Value> = rows
            .into_iter()
            .map(|mut row| {
                if let Some(entity) = row.as_object_mut().and_then(|obj| obj.remove("entity")) {
                    if let Some(entity_obj) = entity.as_object() {
                        if let Some(obj) = row.as_object_mut() {
                            for (k, v) in entity_obj {
                                if !obj.contains_key(k) {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                }
                row
            })
            .collect();
        Ok(flattened)
    }

    pub async fn search_text_dense(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if request.query_vector.is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let filter = doc_filter(&request.auth, request.doc_ids.as_deref());
        let rows = self
            .search_entities(
                &self.config.collection_names().text_chunks,
                "text_dense",
                json!([request.query_vector]),
                filter,
                request.limit,
                &TEXT_OUTPUT_FIELDS,
            )
            .await?;
        let mut chunks = Vec::new();
        for row in rows {
            match scored_text_chunk(row, "milvus_text_dense") {
                Ok(chunk) => chunks.push(chunk),
                Err(e) => eprintln!("[MILVUS WARN] skipped row: {}", e),
            }
        }
        Ok(chunks)
    }

    pub async fn search_bm25(
        &self,
        request: Bm25SearchRequest,
    ) -> anyhow::Result<Bm25SearchOutput> {
        if request.query.trim().is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Bm25SearchOutput {
                chunks: Vec::new(),
                trace: Bm25SearchTrace {
                    backend: "milvus_bm25".to_string(),
                    raw_hit_count: 0,
                    hydrated_hit_count: 0,
                    fallback_reason: None,
                },
            });
        }

        let rows = self
            .search_entities(
                &self.config.collection_names().text_chunks,
                "text_sparse",
                json!([request.query]),
                doc_filter(&request.auth, request.doc_ids.as_deref()),
                request.limit,
                &TEXT_OUTPUT_FIELDS,
            )
            .await?;
        let raw_hit_count = rows.len();
        let mut chunks = Vec::new();
        for row in rows {
            match scored_text_chunk(row, "milvus_bm25") {
                Ok(chunk) => chunks.push(chunk),
                Err(e) => eprintln!("[MILVUS WARN] skipped row: {}", e),
            }
        }
        let hydrated_hit_count = chunks.len();

        Ok(Bm25SearchOutput {
            chunks,
            trace: Bm25SearchTrace {
                backend: "milvus_bm25".to_string(),
                raw_hit_count,
                hydrated_hit_count,
                fallback_reason: None,
            },
        })
    }

    pub async fn search_multimodal(
        &self,
        request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if request.query_vector.is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let rows = self
            .search_entities(
                &self.config.collection_names().multimodal_chunks,
                "multimodal_dense",
                json!([request.query_vector]),
                doc_filter(&request.auth, request.doc_ids.as_deref()),
                request.limit,
                &MULTIMODAL_OUTPUT_FIELDS,
            )
            .await?;
        let mut chunks = Vec::new();
        for row in rows {
            match scored_multimodal_chunk(row, "milvus_multimodal_dense") {
                Ok(chunk) => chunks.push(chunk),
                Err(e) => eprintln!("[MILVUS WARN] skipped row: {}", e),
            }
        }
        Ok(chunks)
    }
}

pub(crate) fn scored_text_chunk(row: Value, channel: &str) -> anyhow::Result<ScoredChunk> {
    let chunk_id = uuid_field(&row, "chunk_id")
        .map_err(|e| anyhow::anyhow!("scored_text_chunk chunk_id error on row {}: {}", row, e))?;
    let doc_id = uuid_field(&row, "doc_id")
        .map_err(|e| anyhow::anyhow!("scored_text_chunk doc_id error on row {}: {}", row, e))?;
    Ok(ScoredChunk {
        chunk_id,
        doc_id,
        content: string_field(&row, "text").unwrap_or_default(),
        score: score_field(&row),
        source: channel.to_string(),
        page: row.get("page").and_then(Value::as_i64),
        chunk_type: string_field(&row, "chunk_type").unwrap_or_else(|| "text".to_string()),
        asset_id: None,
        caption: None,
        image_path: None,
        parser_backend: string_field(&row, "parser_backend"),
        source_locator: row
            .get("source_locator")
            .cloned()
            .filter(|value| !value.is_null()),
        parse_run_id: optional_uuid_field(&row, "parse_run_id")?,
    })
}

pub(crate) fn scored_multimodal_chunk(row: Value, channel: &str) -> anyhow::Result<ScoredChunk> {
    let base_score = score_field(&row);
    let weight = row
        .get("retrieval_weight")
        .and_then(Value::as_f64)
        .map(|w| w as f32)
        .filter(|w| *w > 0.0 && *w < 1.0);
    let score = weight.map(|w| base_score * w).unwrap_or(base_score);

    Ok(ScoredChunk {
        chunk_id: uuid_field(&row, "chunk_id")?,
        doc_id: uuid_field(&row, "doc_id")?,
        content: string_field(&row, "context_text").unwrap_or_default(),
        score,
        source: channel.to_string(),
        page: row.get("page").and_then(Value::as_i64),
        chunk_type: string_field(&row, "chunk_type").unwrap_or_else(|| "multimodal".to_string()),
        asset_id: optional_uuid_field(&row, "asset_id")?,
        caption: string_field(&row, "caption"),
        image_path: string_field(&row, "image_path"),
        parser_backend: string_field(&row, "parser_backend"),
        source_locator: row
            .get("source_locator")
            .cloned()
            .filter(|value| !value.is_null()),
        parse_run_id: optional_uuid_field(&row, "parse_run_id")?,
    })
}
