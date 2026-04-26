use async_trait::async_trait;
use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, DocumentIndexBatch, GraphSearchOutput,
    GraphSearchRequest, IndexWriteReport, MultimodalSearchRequest, RelationPathCandidate,
    RetrievalDataPlane, ScoredChunk, TextDenseSearchRequest,
};
use reqwest::Client;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MilvusConfig {
    pub url: String,
    pub token: Option<String>,
    pub database: Option<String>,
    pub collection_prefix: String,
    pub text_vector_dim: usize,
    pub multimodal_vector_dim: usize,
    pub metric_type: String,
}

impl Default for MilvusConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:19530".to_string(),
            token: None,
            database: Some("default".to_string()),
            collection_prefix: "avrag".to_string(),
            text_vector_dim: 1024,
            multimodal_vector_dim: 1024,
            metric_type: "COSINE".to_string(),
        }
    }
}

impl MilvusConfig {
    pub fn collection_names(&self) -> MilvusCollectionNames {
        let prefix = self.collection_prefix.trim().trim_end_matches('_');
        let prefix = if prefix.is_empty() { "avrag" } else { prefix };
        MilvusCollectionNames {
            text_chunks: format!("{prefix}_rag_text_chunks"),
            multimodal_chunks: format!("{prefix}_rag_multimodal_chunks"),
            kg_entities: format!("{prefix}_rag_kg_entities"),
            kg_relations: format!("{prefix}_rag_kg_relations"),
            graph_passages: format!("{prefix}_rag_graph_passages"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilvusCollectionNames {
    pub text_chunks: String,
    pub multimodal_chunks: String,
    pub kg_entities: String,
    pub kg_relations: String,
    pub graph_passages: String,
}

#[derive(Debug, Clone)]
pub struct MilvusDataPlane {
    config: MilvusConfig,
    client: Client,
}

impl MilvusDataPlane {
    pub fn new(config: MilvusConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn config(&self) -> &MilvusConfig {
        &self.config
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn with_database(&self, mut body: Value) -> Value {
        if let Some(database) = self
            .config
            .database
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            body["dbName"] = json!(database);
        }
        body
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value, MilvusStorageError> {
        let mut request = self.client.post(self.endpoint(path)).json(&body);
        if let Some(token) = self
            .config
            .token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        let status = response.status();
        let body_text = response.text().await?;
        if !status.is_success() {
            return Err(MilvusStorageError::Backend {
                message: format!("Milvus request {path} failed with {status}: {body_text}"),
            });
        }

        let value = serde_json::from_str::<Value>(&body_text).unwrap_or_else(|_| json!({}));
        let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
        if code != 0 && code != 200 {
            return Err(MilvusStorageError::Backend {
                message: value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or(body_text.as_str())
                    .to_string(),
            });
        }
        Ok(value)
    }

    async fn list_collections(&self) -> Result<Vec<String>, MilvusStorageError> {
        let response = self
            .post_json(
                "/v2/vectordb/collections/list",
                self.with_database(json!({})),
            )
            .await?;
        Ok(collection_names_from_response(&response))
    }

    async fn create_collection_if_missing(
        &self,
        existing: &[String],
        name: &str,
        schema: Value,
        index_params: Vec<Value>,
    ) -> Result<(), MilvusStorageError> {
        if existing.iter().any(|existing| existing == name) {
            self.load_collection(name).await?;
            return Ok(());
        }

        self.post_json(
            "/v2/vectordb/collections/create",
            self.with_database(json!({
                "collectionName": name,
                "schema": schema,
                "indexParams": index_params,
                "params": {
                    "consistencyLevel": "Bounded"
                }
            })),
        )
        .await?;
        self.load_collection(name).await
    }

    async fn load_collection(&self, name: &str) -> Result<(), MilvusStorageError> {
        self.post_json(
            "/v2/vectordb/collections/load",
            self.with_database(json!({
                "collectionName": name
            })),
        )
        .await?;
        Ok(())
    }

    async fn insert_entities(
        &self,
        collection: &str,
        rows: Vec<Value>,
    ) -> Result<(), MilvusStorageError> {
        if rows.is_empty() {
            return Ok(());
        }
        self.post_json(
            "/v2/vectordb/entities/insert",
            self.with_database(json!({
                "collectionName": collection,
                "data": rows
            })),
        )
        .await?;
        Ok(())
    }

    async fn delete_by_filter(
        &self,
        collection: &str,
        filter: String,
    ) -> Result<(), MilvusStorageError> {
        self.post_json(
            "/v2/vectordb/entities/delete",
            self.with_database(json!({
                "collectionName": collection,
                "filter": filter
            })),
        )
        .await?;
        Ok(())
    }

    async fn search_entities(
        &self,
        collection: &str,
        anns_field: &str,
        data: Value,
        filter: String,
        limit: usize,
        output_fields: &[&str],
    ) -> Result<Vec<Value>, MilvusStorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let response = self
            .post_json(
                "/v2/vectordb/entities/search",
                self.with_database(json!({
                    "collectionName": collection,
                    "data": data,
                    "annsField": anns_field,
                    "filter": filter,
                    "limit": limit,
                    "outputFields": output_fields,
                    "searchParams": {
                        "metricType": self.config.metric_type
                    }
                })),
            )
            .await?;

        Ok(response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn query_entities(
        &self,
        collection: &str,
        filter: String,
        limit: usize,
        output_fields: &[&str],
    ) -> Result<Vec<Value>, MilvusStorageError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let response = self
            .post_json(
                "/v2/vectordb/entities/query",
                self.with_database(json!({
                    "collectionName": collection,
                    "filter": filter,
                    "limit": limit,
                    "outputFields": output_fields
                })),
            )
            .await?;

        Ok(response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    fn schema_text(&self) -> (Value, Vec<Value>) {
        let schema = collection_schema(
            vec![
                varchar_field("id", 128, true, false, false),
                varchar_field("org_id", 64, false, false, false),
                varchar_field("workspace_id", 64, false, true, false),
                varchar_field("doc_id", 64, false, false, false),
                varchar_field("chunk_id", 64, false, false, false),
                varchar_field("parse_run_id", 64, false, false, false),
                int64_field("doc_version", false),
                int64_field("page", true),
                varchar_field("text", 65_535, false, false, true),
                float_vector_field("text_dense", self.config.text_vector_dim),
                sparse_vector_field("text_sparse"),
                varchar_field("chunk_type", 64, false, false, false),
                varchar_field("parser_backend", 64, false, true, false),
                json_field("source_locator", true),
            ],
            vec![json!({
                "name": "text_bm25",
                "type": "BM25",
                "inputFieldNames": ["text"],
                "outputFieldNames": ["text_sparse"],
                "params": {}
            })],
        );
        let indexes = vec![
            dense_index("text_dense", "text_dense_idx", &self.config.metric_type),
            bm25_index("text_sparse", "text_sparse_idx"),
        ];
        (schema, indexes)
    }

    fn schema_multimodal(&self) -> (Value, Vec<Value>) {
        let schema = collection_schema(
            vec![
                varchar_field("id", 128, true, false, false),
                varchar_field("org_id", 64, false, false, false),
                varchar_field("workspace_id", 64, false, true, false),
                varchar_field("doc_id", 64, false, false, false),
                varchar_field("chunk_id", 64, false, false, false),
                varchar_field("asset_id", 64, false, false, false),
                varchar_field("parse_run_id", 64, false, false, false),
                int64_field("doc_version", false),
                int64_field("page", true),
                varchar_field("context_text", 65_535, false, false, true),
                varchar_field("caption", 4_096, false, true, true),
                varchar_field("image_path", 2_048, false, true, false),
                float_vector_field("multimodal_dense", self.config.multimodal_vector_dim),
                varchar_field("chunk_type", 64, false, false, false),
                varchar_field("parser_backend", 64, false, true, false),
                json_field("source_locator", true),
            ],
            Vec::new(),
        );
        let indexes = vec![dense_index(
            "multimodal_dense",
            "multimodal_dense_idx",
            &self.config.metric_type,
        )];
        (schema, indexes)
    }

    fn schema_entities(&self) -> (Value, Vec<Value>) {
        let schema = collection_schema(
            vec![
                varchar_field("id", 128, true, false, false),
                varchar_field("org_id", 64, false, false, false),
                varchar_field("workspace_id", 64, false, true, false),
                varchar_field("doc_id", 64, false, false, false),
                varchar_field("entity_id", 64, false, false, false),
                varchar_field("parse_run_id", 64, false, false, false),
                int64_field("doc_version", false),
                varchar_field("name", 512, false, false, true),
                varchar_field("normalized_name", 512, false, false, true),
                varchar_field("entity_type", 128, false, true, false),
                float_vector_field("entity_dense", self.config.text_vector_dim),
                json_field("supporting_chunk_ids", false),
                json_field("metadata", true),
            ],
            Vec::new(),
        );
        let indexes = vec![dense_index(
            "entity_dense",
            "entity_dense_idx",
            &self.config.metric_type,
        )];
        (schema, indexes)
    }

    fn schema_relations(&self) -> (Value, Vec<Value>) {
        let schema = collection_schema(
            vec![
                varchar_field("id", 128, true, false, false),
                varchar_field("org_id", 64, false, false, false),
                varchar_field("workspace_id", 64, false, true, false),
                varchar_field("doc_id", 64, false, false, false),
                varchar_field("relation_id", 64, false, false, false),
                varchar_field("parse_run_id", 64, false, false, false),
                int64_field("doc_version", false),
                varchar_field("subject", 512, false, false, true),
                varchar_field("predicate", 256, false, false, true),
                varchar_field("object", 512, false, false, true),
                varchar_field("relation_text", 2_048, false, false, true),
                float_vector_field("relation_dense", self.config.text_vector_dim),
                json_field("supporting_chunk_ids", false),
                json_field("metadata", true),
            ],
            Vec::new(),
        );
        let indexes = vec![dense_index(
            "relation_dense",
            "relation_dense_idx",
            &self.config.metric_type,
        )];
        (schema, indexes)
    }

    fn schema_graph_passages(&self) -> (Value, Vec<Value>) {
        let schema = collection_schema(
            vec![
                varchar_field("id", 128, true, false, false),
                varchar_field("org_id", 64, false, false, false),
                varchar_field("workspace_id", 64, false, true, false),
                varchar_field("doc_id", 64, false, false, false),
                varchar_field("chunk_id", 64, false, true, false),
                varchar_field("passage_id", 64, false, false, false),
                varchar_field("parse_run_id", 64, false, false, false),
                int64_field("doc_version", false),
                varchar_field("text", 65_535, false, false, true),
                float_vector_field("passage_dense", self.config.text_vector_dim),
                json_field("relation_ids", false),
                json_field("metadata", true),
            ],
            Vec::new(),
        );
        let indexes = vec![dense_index(
            "passage_dense",
            "passage_dense_idx",
            &self.config.metric_type,
        )];
        (schema, indexes)
    }
}

#[async_trait]
impl RetrievalDataPlane for MilvusDataPlane {
    async fn ensure_schema(&self) -> anyhow::Result<()> {
        let names = self.config.collection_names();
        let existing = self.list_collections().await?;

        let (schema, indexes) = self.schema_text();
        self.create_collection_if_missing(&existing, &names.text_chunks, schema, indexes)
            .await?;

        let (schema, indexes) = self.schema_multimodal();
        self.create_collection_if_missing(&existing, &names.multimodal_chunks, schema, indexes)
            .await?;

        let (schema, indexes) = self.schema_entities();
        self.create_collection_if_missing(&existing, &names.kg_entities, schema, indexes)
            .await?;

        let (schema, indexes) = self.schema_relations();
        self.create_collection_if_missing(&existing, &names.kg_relations, schema, indexes)
            .await?;

        let (schema, indexes) = self.schema_graph_passages();
        self.create_collection_if_missing(&existing, &names.graph_passages, schema, indexes)
            .await?;

        Ok(())
    }

    async fn replace_document_index(
        &self,
        batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport> {
        self.ensure_schema().await?;
        let auth = AuthContext::new(batch.org_id, avrag_auth::SubjectKind::System);
        self.delete_document_index(&auth, batch.document_id).await?;

        let names = self.config.collection_names();
        let text_count = batch.text_chunks.len();
        let multimodal_count = batch.multimodal_chunks.len();
        let entity_count = batch.entities.len();
        let relation_count = batch.relations.len();
        let graph_passage_count = batch.graph_passages.len();

        self.insert_entities(
            &names.text_chunks,
            batch
                .text_chunks
                .iter()
                .map(|chunk| {
                    json!({
                        "id": chunk.chunk_id.to_string(),
                        "org_id": batch.org_id.to_string(),
                        "workspace_id": batch.workspace_id.as_ref().map(Uuid::to_string),
                        "doc_id": batch.document_id.to_string(),
                        "chunk_id": chunk.chunk_id.to_string(),
                        "parse_run_id": batch.parse_run_id.to_string(),
                        "doc_version": i64::from(batch.doc_version),
                        "page": chunk.page,
                        "text": &chunk.content,
                        "text_dense": &chunk.vector,
                        "chunk_type": &chunk.chunk_type,
                        "parser_backend": &chunk.parser_backend,
                        "source_locator": &chunk.source_locator,
                    })
                })
                .collect(),
        )
        .await?;

        self.insert_entities(
            &names.multimodal_chunks,
            batch
                .multimodal_chunks
                .iter()
                .map(|chunk| {
                    json!({
                        "id": chunk.chunk_id.to_string(),
                        "org_id": batch.org_id.to_string(),
                        "workspace_id": batch.workspace_id.as_ref().map(Uuid::to_string),
                        "doc_id": batch.document_id.to_string(),
                        "chunk_id": chunk.chunk_id.to_string(),
                        "asset_id": chunk.asset_id.to_string(),
                        "parse_run_id": batch.parse_run_id.to_string(),
                        "doc_version": i64::from(batch.doc_version),
                        "page": chunk.page,
                        "context_text": &chunk.context_text,
                        "caption": &chunk.caption,
                        "image_path": &chunk.image_path,
                        "multimodal_dense": &chunk.vector,
                        "chunk_type": &chunk.chunk_type,
                        "parser_backend": &chunk.parser_backend,
                        "source_locator": &chunk.source_locator,
                    })
                })
                .collect(),
        )
        .await?;

        self.insert_entities(
            &names.kg_entities,
            batch
                .entities
                .iter()
                .map(|entity| {
                    json!({
                        "id": entity.entity_id.to_string(),
                        "org_id": batch.org_id.to_string(),
                        "workspace_id": batch.workspace_id.as_ref().map(Uuid::to_string),
                        "doc_id": batch.document_id.to_string(),
                        "entity_id": entity.entity_id.to_string(),
                        "parse_run_id": batch.parse_run_id.to_string(),
                        "doc_version": i64::from(batch.doc_version),
                        "name": &entity.name,
                        "normalized_name": &entity.normalized_name,
                        "entity_type": &entity.entity_type,
                        "entity_dense": &entity.vector,
                        "supporting_chunk_ids": &entity.supporting_chunk_ids,
                        "metadata": &entity.metadata,
                    })
                })
                .collect(),
        )
        .await?;

        self.insert_entities(
            &names.kg_relations,
            batch
                .relations
                .iter()
                .map(|relation| {
                    json!({
                        "id": relation.relation_id.to_string(),
                        "org_id": batch.org_id.to_string(),
                        "workspace_id": batch.workspace_id.as_ref().map(Uuid::to_string),
                        "doc_id": batch.document_id.to_string(),
                        "relation_id": relation.relation_id.to_string(),
                        "parse_run_id": batch.parse_run_id.to_string(),
                        "doc_version": i64::from(batch.doc_version),
                        "subject": &relation.subject,
                        "predicate": &relation.predicate,
                        "object": &relation.object,
                        "relation_text": &relation.relation_text,
                        "relation_dense": &relation.vector,
                        "supporting_chunk_ids": &relation.supporting_chunk_ids,
                        "metadata": &relation.metadata,
                    })
                })
                .collect(),
        )
        .await?;

        self.insert_entities(
            &names.graph_passages,
            batch
                .graph_passages
                .iter()
                .map(|passage| {
                    json!({
                        "id": passage.passage_id.to_string(),
                        "org_id": batch.org_id.to_string(),
                        "workspace_id": batch.workspace_id.as_ref().map(Uuid::to_string),
                        "doc_id": batch.document_id.to_string(),
                        "chunk_id": passage.chunk_id.as_ref().map(Uuid::to_string),
                        "passage_id": passage.passage_id.to_string(),
                        "parse_run_id": batch.parse_run_id.to_string(),
                        "doc_version": i64::from(batch.doc_version),
                        "text": &passage.text,
                        "passage_dense": &passage.vector,
                        "relation_ids": &passage.relation_ids,
                        "metadata": &passage.metadata,
                    })
                })
                .collect(),
        )
        .await?;

        Ok(IndexWriteReport {
            text_chunk_count: text_count,
            multimodal_chunk_count: multimodal_count,
            entity_count,
            relation_count,
            graph_passage_count,
        })
    }

    async fn delete_document_index(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> anyhow::Result<()> {
        let names = self.config.collection_names();
        let filter = doc_filter(auth, Some(&[document_id]));
        for collection in [
            names.text_chunks,
            names.multimodal_chunks,
            names.kg_entities,
            names.kg_relations,
            names.graph_passages,
        ] {
            self.delete_by_filter(&collection, filter.clone()).await?;
        }
        Ok(())
    }

    async fn search_text_dense(
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

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
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

    async fn search_multimodal(
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

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        if request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(GraphSearchOutput::default());
        }

        let Some(filter) = graph_relation_filter(&request) else {
            return Ok(GraphSearchOutput::default());
        };

        let relation_rows = self
            .query_entities(
                &self.config.collection_names().kg_relations,
                filter,
                request.relation_limit,
                &RELATION_OUTPUT_FIELDS,
            )
            .await?;

        let mut relation_paths = Vec::new();
        let mut supporting_chunks = Vec::new();
        for row in relation_rows.into_iter() {
            let candidate = relation_path_candidate(&row)?;
            if supporting_chunks.len() < request.supporting_chunk_limit {
                supporting_chunks.push(scored_relation_chunk(
                    &row,
                    "milvus_graph_relation",
                    supporting_chunks.len(),
                )?);
            }
            relation_paths.push(candidate);
        }

        Ok(GraphSearchOutput {
            relation_paths,
            supporting_chunks,
        })
    }
}

const TEXT_OUTPUT_FIELDS: [&str; 11] = [
    "chunk_id",
    "doc_id",
    "parse_run_id",
    "page",
    "text",
    "chunk_type",
    "parser_backend",
    "source_locator",
    "doc_version",
    "org_id",
    "workspace_id",
];

const MULTIMODAL_OUTPUT_FIELDS: [&str; 14] = [
    "chunk_id",
    "doc_id",
    "asset_id",
    "parse_run_id",
    "page",
    "context_text",
    "caption",
    "image_path",
    "chunk_type",
    "parser_backend",
    "source_locator",
    "doc_version",
    "org_id",
    "workspace_id",
];

const RELATION_OUTPUT_FIELDS: [&str; 13] = [
    "relation_id",
    "doc_id",
    "parse_run_id",
    "subject",
    "predicate",
    "object",
    "relation_text",
    "supporting_chunk_ids",
    "doc_version",
    "org_id",
    "workspace_id",
    "metadata",
    "id",
];

#[derive(Debug, Error)]
pub enum MilvusStorageError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("backend error: {message}")]
    Backend { message: String },
}

fn collection_names_from_response(response: &Value) -> Vec<String> {
    let Some(data) = response.get("data") else {
        return Vec::new();
    };

    if let Some(names) = data.as_array() {
        return names
            .iter()
            .filter_map(|value| {
                value.as_str().map(str::to_string).or_else(|| {
                    value
                        .get("collectionName")
                        .or_else(|| value.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            })
            .collect();
    }

    for key in ["collectionNames", "collections"] {
        if let Some(names) = data.get(key).and_then(Value::as_array) {
            return names
                .iter()
                .filter_map(|value| {
                    value.as_str().map(str::to_string).or_else(|| {
                        value
                            .get("collectionName")
                            .or_else(|| value.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .collect();
        }
    }

    Vec::new()
}

fn collection_schema(fields: Vec<Value>, functions: Vec<Value>) -> Value {
    let mut schema = json!({
        "autoID": false,
        "enableDynamicField": false,
        "fields": fields,
    });
    if !functions.is_empty() {
        schema["functions"] = Value::Array(functions);
    }
    schema
}

fn varchar_field(
    name: &str,
    max_length: usize,
    is_primary: bool,
    nullable: bool,
    enable_analyzer: bool,
) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "VarChar",
        "elementTypeParams": {
            "max_length": max_length,
        }
    });
    if is_primary {
        field["isPrimary"] = json!(true);
    }
    if nullable {
        field["nullable"] = json!(true);
    }
    if enable_analyzer {
        field["elementTypeParams"]["enable_analyzer"] = json!(true);
    }
    field
}

fn int64_field(name: &str, nullable: bool) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "Int64",
    });
    if nullable {
        field["nullable"] = json!(true);
    }
    field
}

fn float_vector_field(name: &str, dim: usize) -> Value {
    json!({
        "fieldName": name,
        "dataType": "FloatVector",
        "elementTypeParams": {
            "dim": dim,
        }
    })
}

fn sparse_vector_field(name: &str) -> Value {
    json!({
        "fieldName": name,
        "dataType": "SparseFloatVector",
    })
}

fn json_field(name: &str, nullable: bool) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "JSON",
    });
    if nullable {
        field["nullable"] = json!(true);
    }
    field
}

fn dense_index(field_name: &str, index_name: &str, metric_type: &str) -> Value {
    json!({
        "fieldName": field_name,
        "indexName": index_name,
        "metricType": metric_type,
        "params": {
            "index_type": "AUTOINDEX"
        }
    })
}

fn bm25_index(field_name: &str, index_name: &str) -> Value {
    json!({
        "fieldName": field_name,
        "indexName": index_name,
        "metricType": "BM25",
        "params": {
            "index_type": "SPARSE_INVERTED_INDEX",
            "inverted_index_algo": "DAAT_MAXSCORE",
            "bm25_k1": 1.2,
            "bm25_b": 0.75
        }
    })
}

fn doc_filter(auth: &AuthContext, doc_ids: Option<&[Uuid]>) -> String {
    let mut filter = format!("org_id == {}", milvus_string(&auth.org_id().to_string()));
    if let Some(doc_ids) = doc_ids {
        let docs = doc_ids
            .iter()
            .map(|doc_id| milvus_string(&doc_id.to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        filter.push_str(&format!(" and doc_id in [{docs}]"));
    }
    filter
}

fn graph_relation_filter(request: &GraphSearchRequest) -> Option<String> {
    let mut predicates = Vec::new();
    let entity_names = normalized_non_empty_values(&request.entity_names);
    if !entity_names.is_empty() {
        let entity_values = entity_names
            .iter()
            .map(|value| milvus_string(value))
            .collect::<Vec<_>>()
            .join(", ");
        predicates.push(format!(
            "(subject in [{entity_values}] or object in [{entity_values}])"
        ));
    }

    for hint in &request.relation_hints {
        let mut parts = Vec::new();
        if let Some(subject) = hint
            .subject
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.push(format!("subject == {}", milvus_string(subject)));
        }
        if let Some(predicate) = hint
            .predicate
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.push(format!("predicate == {}", milvus_string(predicate)));
        }
        if let Some(object) = hint
            .object
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.push(format!("object == {}", milvus_string(object)));
        }
        if !parts.is_empty() {
            predicates.push(format!("({})", parts.join(" and ")));
        }
    }

    if predicates.is_empty() {
        return None;
    }

    let base = doc_filter(&request.auth, request.doc_ids.as_deref());
    Some(format!("{base} and ({})", predicates.join(" or ")))
}

fn normalized_non_empty_values(values: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let key = value.to_lowercase();
            seen.insert(key).then(|| value.to_string())
        })
        .collect()
}

fn milvus_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn scored_text_chunk(row: Value, channel: &str) -> anyhow::Result<ScoredChunk> {
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

fn scored_multimodal_chunk(row: Value, channel: &str) -> anyhow::Result<ScoredChunk> {
    Ok(ScoredChunk {
        chunk_id: uuid_field(&row, "chunk_id")?,
        doc_id: uuid_field(&row, "doc_id")?,
        content: string_field(&row, "context_text").unwrap_or_default(),
        score: score_field(&row),
        source: channel.to_string(),
        page: row.get("page").and_then(Value::as_i64),
        chunk_type: string_field(&row, "chunk_type")
            .unwrap_or_else(|| "image_with_context".to_string()),
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

fn relation_path_candidate(row: &Value) -> anyhow::Result<RelationPathCandidate> {
    Ok(RelationPathCandidate {
        subject: string_field(row, "subject").unwrap_or_default(),
        predicate: string_field(row, "predicate").unwrap_or_default(),
        object: string_field(row, "object").unwrap_or_default(),
        score: graph_score_field(row),
        supporting_chunk_ids: uuid_array_field(row, "supporting_chunk_ids")?,
    })
}

fn scored_relation_chunk(row: &Value, channel: &str, offset: usize) -> anyhow::Result<ScoredChunk> {
    let supporting_chunk_ids = uuid_array_field(row, "supporting_chunk_ids")?;
    let relation_id = optional_uuid_field(row, "relation_id")?;
    let chunk_id = supporting_chunk_ids
        .first()
        .copied()
        .or(relation_id)
        .unwrap_or_else(|| Uuid::new_v4());
    Ok(ScoredChunk {
        chunk_id,
        doc_id: uuid_field(row, "doc_id")?,
        content: string_field(row, "relation_text").unwrap_or_else(|| {
            [
                string_field(row, "subject").unwrap_or_default(),
                string_field(row, "predicate").unwrap_or_default(),
                string_field(row, "object").unwrap_or_default(),
            ]
            .join(" ")
        }),
        score: graph_score_field(row) - (offset as f32 * 0.0001),
        source: channel.to_string(),
        page: None,
        chunk_type: "graph_relation".to_string(),
        asset_id: None,
        caption: None,
        image_path: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: optional_uuid_field(row, "parse_run_id")?,
    })
}

fn uuid_field(row: &Value, key: &str) -> anyhow::Result<Uuid> {
    let value = row
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Milvus row missing {key}"))?;
    Ok(Uuid::parse_str(value)?)
}

fn optional_uuid_field(row: &Value, key: &str) -> anyhow::Result<Option<Uuid>> {
    row.get(key)
        .and_then(Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(Into::into)
}

fn uuid_array_field(row: &Value, key: &str) -> anyhow::Result<Vec<Uuid>> {
    let Some(values) = row.get(key).and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    values
        .iter()
        .filter_map(Value::as_str)
        .map(Uuid::parse_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn string_field(row: &Value, key: &str) -> Option<String> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn score_field(row: &Value) -> f32 {
    row.get("distance")
        .or_else(|| row.get("score"))
        .and_then(Value::as_f64)
        .unwrap_or_default() as f32
}

fn graph_score_field(row: &Value) -> f32 {
    let score = score_field(row);
    if score > 0.0 { score } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_names_apply_prefix_once() {
        let config = MilvusConfig {
            collection_prefix: "demo_".to_string(),
            ..MilvusConfig::default()
        };
        assert_eq!(
            config.collection_names().text_chunks,
            "demo_rag_text_chunks"
        );
    }

    #[test]
    fn doc_filter_includes_org_and_doc_scope() {
        let auth = AuthContext::new(
            avrag_auth::OrgId::new(Uuid::from_u128(1)),
            avrag_auth::SubjectKind::System,
        );
        let doc_id = Uuid::from_u128(2);
        assert_eq!(
            doc_filter(&auth, Some(&[doc_id])),
            "org_id == \"00000000-0000-0000-0000-000000000001\" and doc_id in [\"00000000-0000-0000-0000-000000000002\"]"
        );
    }

    #[test]
    fn text_schema_declares_bm25_function() {
        let adapter = MilvusDataPlane::new(MilvusConfig::default());
        let (schema, indexes) = adapter.schema_text();
        assert_eq!(schema["functions"][0]["type"], "BM25");
        assert!(indexes.iter().any(|index| index["metricType"] == "BM25"));
    }

    #[test]
    fn graph_relation_filter_uses_entity_names_and_hints() {
        let auth = AuthContext::new(
            avrag_auth::OrgId::new(Uuid::from_u128(1)),
            avrag_auth::SubjectKind::System,
        );
        let request = GraphSearchRequest {
            auth,
            doc_ids: Some(vec![Uuid::from_u128(2)]),
            entity_names: vec!["Atlas".to_string()],
            relation_hints: vec![avrag_retrieval_data_plane::GraphRelationHint {
                subject: Some("Atlas".to_string()),
                predicate: Some("uses".to_string()),
                object: Some("rollback checklist".to_string()),
            }],
            relation_limit: 20,
            supporting_chunk_limit: 8,
        };

        let filter = graph_relation_filter(&request).unwrap();

        assert!(filter.contains("org_id =="));
        assert!(filter.contains("doc_id in"));
        assert!(filter.contains("subject in [\"Atlas\"]"));
        assert!(filter.contains("predicate == \"uses\""));
    }

    #[test]
    fn relation_row_maps_to_path_and_supporting_chunk() {
        let chunk_id = Uuid::from_u128(10);
        let row = json!({
            "relation_id": Uuid::from_u128(11).to_string(),
            "doc_id": Uuid::from_u128(12).to_string(),
            "parse_run_id": Uuid::from_u128(13).to_string(),
            "subject": "Atlas",
            "predicate": "uses",
            "object": "rollback checklist",
            "relation_text": "Atlas uses rollback checklist",
            "supporting_chunk_ids": [chunk_id.to_string()]
        });

        let path = relation_path_candidate(&row).unwrap();
        let chunk = scored_relation_chunk(&row, "milvus_graph_relation", 0).unwrap();

        assert_eq!(path.supporting_chunk_ids, vec![chunk_id]);
        assert_eq!(chunk.chunk_id, chunk_id);
        assert_eq!(chunk.chunk_type, "graph_relation");
        assert_eq!(chunk.parse_run_id, Some(Uuid::from_u128(13)));
    }
}
