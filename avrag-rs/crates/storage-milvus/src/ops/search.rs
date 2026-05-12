use serde_json::{Value, json};
use avrag_retrieval_data_plane::{Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, ScoredChunk, TextDenseSearchRequest, MultimodalSearchRequest};
use crate::lib_impl::MilvusDataPlane;
use crate::schema::{TEXT_OUTPUT_FIELDS, MULTIMODAL_OUTPUT_FIELDS, doc_filter};
use crate::utils::{uuid_field, optional_uuid_field, string_field, score_field};
use crate::types::Result;

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
        let response = self
            .post_json(
                "/v2/vectordb/entities/search",
                self.with_database(json!({
                    "collectionName": collection,
                    "annsField": vector_field,
                    "data": data,
                    "filter": filter,
                    "limit": limit,
                    "outputFields": output_fields
                })),
            )
            .await?;
        Ok(response["data"].as_array().cloned().unwrap_or_default())
    }

    pub async fn search_text_dense(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if request.query_vector.is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let rows = self
            .search_entities(
                &self.config.collection_names().text_chunks,
                "text_dense",
                json!([request.query_vector]),
                doc_filter(&request.auth, request.doc_ids.as_deref()),
                request.limit,
                &TEXT_OUTPUT_FIELDS,
            )
            .await?;
        rows.into_iter()
            .map(|row| scored_text_chunk(row, "milvus_text_dense"))
            .collect()
    }

    pub async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
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
        let chunks = rows
            .into_iter()
            .map(|row| scored_text_chunk(row, "milvus_bm25"))
            .collect::<anyhow::Result<Vec<_>>>()?;
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
        rows.into_iter()
            .map(|row| scored_multimodal_chunk(row, "milvus_multimodal_dense"))
            .collect()
    }
}

pub(crate) fn scored_text_chunk(row: Value, channel: &str) -> anyhow::Result<ScoredChunk> {
    Ok(ScoredChunk {
        chunk_id: uuid_field(&row, "chunk_id")?,
        doc_id: uuid_field(&row, "doc_id")?,
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
    Ok(ScoredChunk {
        chunk_id: uuid_field(&row, "chunk_id")?,
        doc_id: uuid_field(&row, "doc_id")?,
        content: string_field(&row, "context_text").unwrap_or_default(),
        score: score_field(&row),
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
