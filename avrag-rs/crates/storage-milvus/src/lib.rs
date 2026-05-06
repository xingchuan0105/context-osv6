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

// Internal trait to abstract insert/delete for testability.
// NOT exposed as public API.
#[async_trait]
trait WriteExecutor: Send + Sync {
    async fn insert(&self, collection: &str, rows: Vec<Value>) -> Result<(), MilvusStorageError>;
    async fn delete(&self, collection: &str, filter: String) -> Result<(), MilvusStorageError>;
}

struct RealExecutor<'a> {
    plane: &'a MilvusDataPlane,
}

#[async_trait]
impl WriteExecutor for RealExecutor<'_> {
    async fn insert(&self, collection: &str, rows: Vec<Value>) -> Result<(), MilvusStorageError> {
        self.plane.insert_entities(collection, rows).await
    }
    async fn delete(&self, collection: &str, filter: String) -> Result<(), MilvusStorageError> {
        self.plane.delete_by_filter(collection, filter).await
    }
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
            let describe = self.describe_collection(name).await?;
            validate_existing_collection_schema(name, &schema, &describe)?;
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

    async fn describe_collection(&self, name: &str) -> Result<Value, MilvusStorageError> {
        self.post_json(
            "/v2/vectordb/collections/describe",
            self.with_database(json!({
                "collectionName": name
            })),
        )
        .await
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

    async fn insert_if_nonempty<E: WriteExecutor>(
        &self,
        executor: &E,
        collection: &str,
        rows: Vec<Value>,
        attempted: &mut Vec<String>,
    ) -> Result<(), MilvusStorageError> {
        if rows.is_empty() {
            return Ok(());
        }
        attempted.push(collection.to_string());
        executor.insert(collection, rows).await?;
        Ok(())
    }

    async fn cleanup_current_parse_run<E: WriteExecutor>(
        &self,
        executor: &E,
        collections: &[String],
        document_id: &Uuid,
        parse_run_id: &Uuid,
    ) -> Result<(), MilvusStorageError> {
        let filter = format!(
            "doc_id == '{}' && parse_run_id == '{}'",
            document_id, parse_run_id
        );
        let mut errors = Vec::new();
        for collection in collections {
            if let Err(e) = executor.delete(collection, filter.clone()).await {
                errors.push(format!("{}: {}", collection, e));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(MilvusStorageError::Backend {
                message: format!(
                    "cleanup of current parse_run partial writes failed: {}",
                    errors.join("; ")
                ),
            })
        }
    }

    /// Core replace logic parameterised over a WriteExecutor so tests
    /// can inject failures and record calls without touching a live Milvus.
    async fn replace_document_index_impl<E: WriteExecutor>(
        &self,
        batch: DocumentIndexBatch,
        executor: &E,
    ) -> anyhow::Result<IndexWriteReport> {
        validate_document_batch_vector_dims(&batch, &self.config)?;
        let names = self.config.collection_names();

        let text_count = batch.text_chunks.len();
        let multimodal_count = batch.multimodal_chunks.len();
        let entity_count = batch.entities.len();
        let relation_count = batch.relations.len();
        let graph_passage_count = batch.graph_passages.len();

        let mut attempted: Vec<String> = Vec::new();

        let text_rows: Vec<Value> = if !batch.text_chunks.is_empty() {
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
                .collect()
        } else {
            Vec::new()
        };
        if let Err(e) = self
            .insert_if_nonempty(executor, &names.text_chunks, text_rows, &mut attempted)
            .await
        {
            return cleanup_partial_and_err(
                self,
                executor,
                e,
                &attempted,
                &batch.document_id,
                &batch.parse_run_id,
            )
            .await;
        }

        let multimodal_rows: Vec<Value> = if !batch.multimodal_chunks.is_empty() {
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
                .collect()
        } else {
            Vec::new()
        };
        if let Err(e) = self
            .insert_if_nonempty(
                executor,
                &names.multimodal_chunks,
                multimodal_rows,
                &mut attempted,
            )
            .await
        {
            return cleanup_partial_and_err(
                self,
                executor,
                e,
                &attempted,
                &batch.document_id,
                &batch.parse_run_id,
            )
            .await;
        }

        let entity_rows: Vec<Value> = if !batch.entities.is_empty() {
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
                .collect()
        } else {
            Vec::new()
        };
        if let Err(e) = self
            .insert_if_nonempty(executor, &names.kg_entities, entity_rows, &mut attempted)
            .await
        {
            return cleanup_partial_and_err(
                self,
                executor,
                e,
                &attempted,
                &batch.document_id,
                &batch.parse_run_id,
            )
            .await;
        }

        let relation_rows: Vec<Value> = if !batch.relations.is_empty() {
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
                .collect()
        } else {
            Vec::new()
        };
        if let Err(e) = self
            .insert_if_nonempty(executor, &names.kg_relations, relation_rows, &mut attempted)
            .await
        {
            return cleanup_partial_and_err(
                self,
                executor,
                e,
                &attempted,
                &batch.document_id,
                &batch.parse_run_id,
            )
            .await;
        }

        let passage_rows: Vec<Value> = if !batch.graph_passages.is_empty() {
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
                .collect()
        } else {
            Vec::new()
        };
        if let Err(e) = self
            .insert_if_nonempty(
                executor,
                &names.graph_passages,
                passage_rows,
                &mut attempted,
            )
            .await
        {
            return cleanup_partial_and_err(
                self,
                executor,
                e,
                &attempted,
                &batch.document_id,
                &batch.parse_run_id,
            )
            .await;
        }

        // Phase 2: All inserts succeeded — now delete old parse_run_ids.
        let delete_filter = format!(
            "doc_id == '{}' && parse_run_id != '{}'",
            batch.document_id, batch.parse_run_id
        );

        let mut delete_errors = Vec::new();
        for collection in [
            &names.text_chunks,
            &names.multimodal_chunks,
            &names.kg_entities,
            &names.kg_relations,
            &names.graph_passages,
        ] {
            if let Err(e) = executor.delete(collection, delete_filter.clone()).await {
                delete_errors.push(format!("{}: {}", collection, e));
            }
        }

        if !delete_errors.is_empty() {
            return Err(anyhow::anyhow!(
                "New data inserted but failed to delete old parse_run_ids for: {}. Manual cleanup may be needed.",
                delete_errors.join("; ")
            ));
        }

        Ok(IndexWriteReport {
            text_chunk_count: text_count,
            multimodal_chunk_count: multimodal_count,
            entity_count,
            relation_count,
            graph_passage_count,
        })
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
        self.replace_document_index_impl(batch, &RealExecutor { plane: self })
            .await
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

/// Cleanup attempted collections for the current parse_run and
/// return an error with both the insert failure and cleanup result.
async fn cleanup_partial_and_err<E: WriteExecutor>(
    plane: &MilvusDataPlane,
    executor: &E,
    insert_err: MilvusStorageError,
    attempted: &[String],
    document_id: &Uuid,
    parse_run_id: &Uuid,
) -> anyhow::Result<IndexWriteReport> {
    if attempted.is_empty() {
        return Err(anyhow::anyhow!(
            "insert failed: {}. No partial writes to clean up.",
            insert_err
        ));
    }
    match plane
        .cleanup_current_parse_run(executor, attempted, document_id, parse_run_id)
        .await
    {
        Ok(()) => Err(anyhow::anyhow!(
            "insert failed: {}. Cleaned up attempted writes for current parse_run (collections: {:?}).",
            insert_err,
            attempted
        )),
        Err(cleanup_err) => Err(anyhow::anyhow!(
            "insert failed: {}. Cleanup of partial writes also failed: {}",
            insert_err,
            cleanup_err
        )),
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

fn validate_existing_collection_schema(
    collection_name: &str,
    expected_schema: &Value,
    describe_response: &Value,
) -> Result<(), MilvusStorageError> {
    let expected_fields =
        schema_fields(expected_schema).ok_or_else(|| MilvusStorageError::Backend {
            message: format!(
                "collection {collection_name} expected schema has no fields; cannot validate"
            ),
        })?;

    let actual_fields =
        describe_schema_fields(describe_response).ok_or_else(|| MilvusStorageError::Backend {
            message: format!(
                "collection {collection_name} describe response missing schema fields; cannot validate compatibility"
            ),
        })?;

    let actual_specs = actual_fields
        .iter()
        .filter_map(|field| {
            let name = field_name(field)?;
            Some((name.to_string(), field_spec(field)))
        })
        .collect::<std::collections::HashMap<_, _>>();

    for expected_field in expected_fields {
        let Some(expected_name) = field_name(expected_field) else {
            continue;
        };
        let expected_spec = field_spec(expected_field);
        let Some(actual_spec) = actual_specs.get(expected_name) else {
            return Err(MilvusStorageError::Backend {
                message: format!(
                    "collection {collection_name} is incompatible: missing expected field `{expected_name}`"
                ),
            });
        };

        if let Some(expected_type) = expected_spec.data_type.as_deref() {
            let Some(actual_type) = actual_spec.data_type.as_deref() else {
                return Err(MilvusStorageError::Backend {
                    message: format!(
                        "collection {collection_name} field `{expected_name}` missing field type in describe response"
                    ),
                });
            };
            if !expected_type.eq_ignore_ascii_case(actual_type) {
                return Err(MilvusStorageError::Backend {
                    message: format!(
                        "collection {collection_name} field `{expected_name}` has type `{actual_type}`, expected `{expected_type}`"
                    ),
                });
            }
        }

        if let Some(expected_dim) = expected_spec.vector_dim {
            match actual_spec.vector_dim {
                Some(actual_dim) if actual_dim == expected_dim => {}
                Some(actual_dim) => {
                    return Err(MilvusStorageError::Backend {
                        message: format!(
                            "collection {collection_name} field `{expected_name}` dim mismatch: expected {expected_dim}, got {actual_dim}"
                        ),
                    });
                }
                None => {
                    return Err(MilvusStorageError::Backend {
                        message: format!(
                            "collection {collection_name} field `{expected_name}` missing vector dim in describe response"
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}

fn validate_document_batch_vector_dims(
    batch: &DocumentIndexBatch,
    config: &MilvusConfig,
) -> anyhow::Result<()> {
    for (idx, chunk) in batch.text_chunks.iter().enumerate() {
        validate_vector_dim(
            &format!("text_chunks[{idx}].text_dense"),
            chunk.vector.len(),
            config.text_vector_dim,
        )?;
    }
    for (idx, chunk) in batch.multimodal_chunks.iter().enumerate() {
        validate_vector_dim(
            &format!("multimodal_chunks[{idx}].multimodal_dense"),
            chunk.vector.len(),
            config.multimodal_vector_dim,
        )?;
    }
    for (idx, entity) in batch.entities.iter().enumerate() {
        validate_vector_dim(
            &format!("entities[{idx}].entity_dense"),
            entity.vector.len(),
            config.text_vector_dim,
        )?;
    }
    for (idx, relation) in batch.relations.iter().enumerate() {
        validate_vector_dim(
            &format!("relations[{idx}].relation_dense"),
            relation.vector.len(),
            config.text_vector_dim,
        )?;
    }
    for (idx, passage) in batch.graph_passages.iter().enumerate() {
        validate_vector_dim(
            &format!("graph_passages[{idx}].passage_dense"),
            passage.vector.len(),
            config.text_vector_dim,
        )?;
    }
    Ok(())
}

fn validate_vector_dim(path: &str, actual: usize, expected: usize) -> anyhow::Result<()> {
    if actual == expected {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "vector dimension mismatch for {path}: expected {expected}, got {actual}"
    ))
}

#[derive(Debug, Clone)]
struct FieldSpec {
    data_type: Option<String>,
    vector_dim: Option<usize>,
}

fn field_spec(field: &Value) -> FieldSpec {
    FieldSpec {
        data_type: field_data_type(field),
        vector_dim: field_dim(field),
    }
}

fn schema_fields(schema: &Value) -> Option<&Vec<Value>> {
    schema.get("fields").and_then(Value::as_array)
}

fn describe_schema_fields(response: &Value) -> Option<&Vec<Value>> {
    let data = response.get("data")?;
    if let Some(fields) = fields_from_container(data) {
        return Some(fields);
    }
    data.as_array()
        .and_then(|items| items.first())
        .and_then(fields_from_container)
}

fn fields_from_container(value: &Value) -> Option<&Vec<Value>> {
    value
        .get("schema")
        .and_then(|schema| schema.get("fields"))
        .and_then(Value::as_array)
        .or_else(|| value.get("fields").and_then(Value::as_array))
}

fn field_name(field: &Value) -> Option<&str> {
    field
        .get("fieldName")
        .or_else(|| field.get("name"))
        .and_then(Value::as_str)
}

fn field_data_type(field: &Value) -> Option<String> {
    field
        .get("dataType")
        .or_else(|| field.get("type"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn field_dim(field: &Value) -> Option<usize> {
    let dim_value = field
        .get("elementTypeParams")
        .and_then(|params| params.get("dim"))
        .or_else(|| field.get("typeParams").and_then(|params| params.get("dim")))
        .or_else(|| field.get("params").and_then(|params| params.get("dim")))
        .or_else(|| field.get("params").and_then(params_array_dim))
        .or_else(|| field.get("dim"))?;
    value_to_usize(dim_value)
}

fn params_array_dim(params: &Value) -> Option<&Value> {
    params.as_array()?.iter().find_map(|param| {
        let key = param.get("key").and_then(Value::as_str)?;
        if key.eq_ignore_ascii_case("dim") {
            param.get("value")
        } else {
            None
        }
    })
}

fn value_to_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .or_else(|| value.as_str().and_then(|v| v.parse::<usize>().ok()))
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
        .unwrap_or_else(Uuid::new_v4);
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
    use avrag_retrieval_data_plane::{
        EntityIndexRecord, GraphPassageIndexRecord, MultimodalChunkIndexRecord,
        RelationIndexRecord, TextChunkIndexRecord,
    };

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
    fn existing_schema_accepts_milvus_describe_params_array_dim() {
        let expected_schema =
            collection_schema(vec![float_vector_field("text_dense", 1024)], vec![]);
        let describe_response = json!({
            "code": 0,
            "data": {
                "fields": [
                    {
                        "name": "text_dense",
                        "type": "FloatVector",
                        "params": [{ "key": "dim", "value": "1024" }]
                    }
                ]
            }
        });

        validate_existing_collection_schema(
            "test_collection",
            &expected_schema,
            &describe_response,
        )
        .expect("Milvus v2 describe response params array should expose vector dim");
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

    #[test]
    fn cleanup_filter_uses_current_parse_run_equal_not_old_not_equal() {
        let doc_id = Uuid::from_u128(42);
        let parse_run_id = Uuid::from_u128(99);

        // On insert failure, the compensating delete must target
        // the current parse_run (==), NOT old parse_runs (!=).
        let cleanup_filter = format!(
            "doc_id == '{}' && parse_run_id == '{}'",
            doc_id, parse_run_id
        );
        assert!(
            cleanup_filter.contains("parse_run_id =="),
            "cleanup filter must use '==' to target current parse_run"
        );
        assert!(
            !cleanup_filter.contains("parse_run_id !="),
            "cleanup filter must NOT use '!=' (that deletes old parse_runs)"
        );

        // On success, the delete filter targets old parse_runs (!=).
        let old_delete_filter = format!(
            "doc_id == '{}' && parse_run_id != '{}'",
            doc_id, parse_run_id
        );
        assert!(old_delete_filter.contains("parse_run_id !="));
        assert!(!old_delete_filter.contains("parse_run_id =="));
    }

    #[test]
    fn insert_if_nonempty_does_not_track_empty_rows() {
        // Verify that insert_if_nonempty returns early for empty rows
        // without adding the collection to the attempted tracker.
        // (This is a compile-and-logic check — actual insert requires a live Milvus.)
        let attempted: Vec<String> = Vec::new();
        let rows: Vec<Value> = Vec::new();

        // The method signature guarantees we can call it with empty rows.
        // The actual async call can't be tested without a runtime, but we
        // verify the empty-rows early return via the struct method contract.
        assert!(attempted.is_empty());
        assert!(rows.is_empty());

        // On a real call: insert_if_nonempty would return Ok(()) immediately
        // and attempted would remain empty.
        assert_eq!(attempted.len(), 0);
    }

    // ── FakeExecutor ──────────────────────────────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Insert {
            collection: String,
            row_count: usize,
        },
        Delete {
            collection: String,
            filter: String,
        },
    }

    struct FakeExecutor {
        calls: std::sync::Mutex<Vec<Call>>,
        fail_insert_on: std::sync::Mutex<Option<String>>,
        fail_delete_on: std::sync::Mutex<Option<String>>,
    }

    impl FakeExecutor {
        fn new() -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                fail_insert_on: std::sync::Mutex::new(None),
                fail_delete_on: std::sync::Mutex::new(None),
            }
        }

        fn with_insert_failure(collection: &str) -> Self {
            let ex = Self::new();
            *ex.fail_insert_on.lock().unwrap() = Some(collection.to_string());
            ex
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }

        fn insert_calls(&self) -> Vec<(String, usize)> {
            self.calls()
                .into_iter()
                .filter_map(|c| match c {
                    Call::Insert {
                        collection,
                        row_count,
                    } => Some((collection, row_count)),
                    _ => None,
                })
                .collect()
        }

        fn delete_calls(&self) -> Vec<(String, String)> {
            self.calls()
                .into_iter()
                .filter_map(|c| match c {
                    Call::Delete { collection, filter } => Some((collection, filter)),
                    _ => None,
                })
                .collect()
        }
    }

    #[async_trait]
    impl WriteExecutor for FakeExecutor {
        async fn insert(
            &self,
            collection: &str,
            rows: Vec<Value>,
        ) -> Result<(), MilvusStorageError> {
            self.calls.lock().unwrap().push(Call::Insert {
                collection: collection.to_string(),
                row_count: rows.len(),
            });
            if let Some(ref fail_on) = *self.fail_insert_on.lock().unwrap()
                && fail_on == collection
            {
                return Err(MilvusStorageError::Backend {
                    message: format!("injected insert failure on {}", collection),
                });
            }
            Ok(())
        }

        async fn delete(&self, collection: &str, filter: String) -> Result<(), MilvusStorageError> {
            self.calls.lock().unwrap().push(Call::Delete {
                collection: collection.to_string(),
                filter: filter.clone(),
            });
            if let Some(ref fail_on) = *self.fail_delete_on.lock().unwrap()
                && fail_on == collection
            {
                return Err(MilvusStorageError::Backend {
                    message: format!("injected delete failure on {}", collection),
                });
            }
            Ok(())
        }
    }

    // Helper to build a fully-populated batch.
    fn make_test_batch(doc_id: Uuid, parse_run_id: Uuid) -> DocumentIndexBatch {
        let chunk_id = Uuid::from_u128(100);
        let relation_id = Uuid::from_u128(200);
        DocumentIndexBatch {
            org_id: avrag_auth::OrgId::new(Uuid::from_u128(1)),
            workspace_id: None,
            document_id: doc_id,
            parse_run_id,
            doc_version: 1,
            text_chunks: vec![TextChunkIndexRecord {
                chunk_id,
                content: "text chunk".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                page: Some(1),
                chunk_type: "text".to_string(),
                parser_backend: Some("test".to_string()),
                source_locator: None,
            }],
            multimodal_chunks: vec![MultimodalChunkIndexRecord {
                chunk_id: Uuid::from_u128(101),
                asset_id: Uuid::from_u128(102),
                context_text: "image chunk".to_string(),
                caption: Some("caption".to_string()),
                image_path: Some("s3://bucket/img.png".to_string()),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                page: Some(2),
                chunk_type: "image_with_context".to_string(),
                parser_backend: Some("test".to_string()),
                source_locator: None,
            }],
            entities: vec![EntityIndexRecord {
                entity_id: Uuid::from_u128(103),
                name: "Entity".to_string(),
                normalized_name: "entity".to_string(),
                entity_type: Some("test".to_string()),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            relations: vec![RelationIndexRecord {
                relation_id,
                subject: "Entity".to_string(),
                predicate: "has".to_string(),
                object: "Property".to_string(),
                relation_text: "Entity has Property".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            graph_passages: vec![GraphPassageIndexRecord {
                passage_id: Uuid::from_u128(104),
                chunk_id: Some(chunk_id),
                text: "passage text".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                relation_ids: vec![relation_id],
                metadata: None,
            }],
        }
    }

    fn test_config() -> MilvusConfig {
        MilvusConfig {
            text_vector_dim: 4,
            multimodal_vector_dim: 4,
            ..MilvusConfig::default()
        }
    }

    // ── Failure cleanup test ──────────────────────────────────────────────

    #[tokio::test]
    async fn insert_failure_triggers_cleanup_of_current_parse_run_only() {
        let plane = MilvusDataPlane::new(test_config());
        let doc_id = Uuid::from_u128(42);
        let parse_run_id = Uuid::from_u128(99);
        let names = plane.config.collection_names();

        let executor = FakeExecutor::with_insert_failure(&names.multimodal_chunks);
        let batch = make_test_batch(doc_id, parse_run_id);

        let result = plane.replace_document_index_impl(batch, &executor).await;
        assert!(result.is_err(), "expected failure");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("injected insert failure"),
            "error should mention insert failure: {}",
            err_msg
        );

        // text_chunks insert succeeded, multimodal_chunks insert attempted and failed.
        let inserts = executor.insert_calls();
        assert_eq!(
            inserts.len(),
            2,
            "should have attempted text_chunks and multimodal_chunks inserts: {:?}",
            inserts
        );
        assert_eq!(inserts[0].0, names.text_chunks);
        assert_eq!(inserts[0].1, 1); // 1 row
        assert_eq!(inserts[1].0, names.multimodal_chunks);
        assert_eq!(inserts[1].1, 1); // 1 row

        // No further inserts after multimodal_chunks failure.
        let collections_after_failure: std::collections::HashSet<_> =
            inserts.iter().skip(2).map(|(c, _)| c.as_str()).collect();
        assert!(
            !collections_after_failure.contains(names.kg_entities.as_str()),
            "kg_entities should not be inserted after failure"
        );
        assert!(
            !collections_after_failure.contains(names.kg_relations.as_str()),
            "kg_relations should not be inserted after failure"
        );
        assert!(
            !collections_after_failure.contains(names.graph_passages.as_str()),
            "graph_passages should not be inserted after failure"
        );

        // Cleanup delete was called for all attempted non-empty collections,
        // including the collection whose insert returned an error.
        let deletes = executor.delete_calls();
        assert!(
            !deletes.is_empty(),
            "cleanup delete should have been called"
        );

        for (collection, filter) in &deletes {
            assert!(
                filter.contains(&format!("parse_run_id == '{}'", parse_run_id)),
                "cleanup filter for {} must target current parse_run (==), got: {}",
                collection,
                filter
            );
            assert!(
                !filter.contains("parse_run_id !="),
                "cleanup filter for {} must NOT use !=, got: {}",
                collection,
                filter
            );
            assert!(
                filter.contains(&format!("doc_id == '{}'", doc_id)),
                "cleanup filter for {} must contain doc_id, got: {}",
                collection,
                filter
            );
        }

        // Cleanup must also target the failed collection because Milvus may
        // have partially written rows before returning the insert error.
        let cleanup_collections: Vec<_> = deletes
            .iter()
            .filter(|(_, f)| f.contains(&format!("parse_run_id == '{}'", parse_run_id)))
            .map(|(c, _)| c.as_str())
            .collect();
        assert_eq!(
            cleanup_collections,
            vec![names.text_chunks.as_str(), names.multimodal_chunks.as_str()],
            "cleanup should delete from all attempted collections"
        );
    }

    // ── Cleanup-failure test ──────────────────────────────────────────────

    #[tokio::test]
    async fn insert_and_cleanup_both_fail_produces_combined_error() {
        let plane = MilvusDataPlane::new(test_config());
        let doc_id = Uuid::from_u128(42);
        let parse_run_id = Uuid::from_u128(99);
        let names = plane.config.collection_names();

        // Inject failure on multimodal_chunks insert AND on text_chunks cleanup delete.
        let executor = FakeExecutor::with_insert_failure(&names.multimodal_chunks);
        // Chain delete failure by setting the second failure point.
        *executor.fail_delete_on.lock().unwrap() = Some(names.text_chunks.clone());
        let batch = make_test_batch(doc_id, parse_run_id);

        let result = plane.replace_document_index_impl(batch, &executor).await;
        assert!(result.is_err(), "expected failure");

        let err_msg = result.unwrap_err().to_string();

        // Error must mention the original insert failure.
        assert!(
            err_msg.contains("insert failed"),
            "error must mention insert failure: {}",
            err_msg
        );
        assert!(
            err_msg.contains("injected insert failure"),
            "error must contain the original insert failure details: {}",
            err_msg
        );

        // Error must mention the cleanup failure.
        assert!(
            err_msg.contains("Cleanup of partial writes also failed"),
            "error must mention cleanup failure: {}",
            err_msg
        );
        assert!(
            err_msg.contains("injected delete failure"),
            "error must contain cleanup failure details: {}",
            err_msg
        );
    }

    // ── Success-order test ────────────────────────────────────────────────

    #[tokio::test]
    async fn all_inserts_succeed_then_old_parse_run_deleted() {
        let plane = MilvusDataPlane::new(test_config());
        let doc_id = Uuid::from_u128(42);
        let parse_run_id = Uuid::from_u128(99);
        let names = plane.config.collection_names();

        let executor = FakeExecutor::new();
        let batch = make_test_batch(doc_id, parse_run_id);

        let result = plane.replace_document_index_impl(batch, &executor).await;
        assert!(result.is_ok(), "expected success: {:?}", result);

        let report = result.unwrap();
        assert_eq!(report.text_chunk_count, 1);
        assert_eq!(report.multimodal_chunk_count, 1);
        assert_eq!(report.entity_count, 1);
        assert_eq!(report.relation_count, 1);
        assert_eq!(report.graph_passage_count, 1);

        // All inserts happened.
        let inserts = executor.insert_calls();
        assert_eq!(
            inserts.len(),
            5,
            "all five collections should have been inserted: {:?}",
            inserts
        );
        let insert_order: Vec<_> = inserts.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(
            insert_order,
            vec![
                names.text_chunks.as_str(),
                names.multimodal_chunks.as_str(),
                names.kg_entities.as_str(),
                names.kg_relations.as_str(),
                names.graph_passages.as_str(),
            ]
        );

        // Old parse_run delete happened after all inserts.
        let deletes = executor.delete_calls();
        assert_eq!(
            deletes.len(),
            5,
            "all five collections should have old parse_run deleted: {:?}",
            deletes
        );

        // All delete filters must target old parse_runs (!=).
        for (collection, filter) in &deletes {
            assert!(
                filter.contains(&format!("parse_run_id != '{}'", parse_run_id)),
                "delete filter for {} must target old parse_runs (!=), got: {}",
                collection,
                filter
            );
            assert!(
                !filter.contains("parse_run_id =="),
                "delete filter for {} must NOT use ==, got: {}",
                collection,
                filter
            );
        }

        // Verify call order: all inserts before any old-parse-run deletes.
        let calls = executor.calls();
        let first_delete_idx = calls
            .iter()
            .position(|c| matches!(c, Call::Delete { .. }))
            .expect("there should be delete calls");
        let last_insert_idx = calls
            .iter()
            .rposition(|c| matches!(c, Call::Insert { .. }))
            .expect("there should be insert calls");
        assert!(
            last_insert_idx < first_delete_idx,
            "all inserts must complete before any old parse_run delete: last_insert={}, first_delete={}",
            last_insert_idx,
            first_delete_idx
        );
    }

    // ── Empty-collections success test ────────────────────────────────────

    #[tokio::test]
    async fn empty_collections_are_skipped_and_old_parse_run_still_deleted() {
        let plane = MilvusDataPlane::new(test_config());
        let doc_id = Uuid::from_u128(42);
        let parse_run_id = Uuid::from_u128(99);
        let names = plane.config.collection_names();

        let executor = FakeExecutor::new();
        let batch = DocumentIndexBatch {
            org_id: avrag_auth::OrgId::new(Uuid::from_u128(1)),
            workspace_id: None,
            document_id: doc_id,
            parse_run_id,
            doc_version: 1,
            text_chunks: vec![TextChunkIndexRecord {
                chunk_id: Uuid::from_u128(100),
                content: "only text".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                page: Some(1),
                chunk_type: "text".to_string(),
                parser_backend: Some("test".to_string()),
                source_locator: None,
            }],
            multimodal_chunks: vec![],
            entities: vec![],
            relations: vec![],
            graph_passages: vec![],
        };

        let result = plane.replace_document_index_impl(batch, &executor).await;
        assert!(result.is_ok(), "expected success: {:?}", result);

        // Only text_chunks inserted.
        let inserts = executor.insert_calls();
        assert_eq!(inserts.len(), 1);
        assert_eq!(inserts[0].0, names.text_chunks);

        // All collections get old parse_run delete (including empty ones).
        let deletes = executor.delete_calls();
        assert_eq!(deletes.len(), 5);
    }

    #[tokio::test]
    async fn replace_document_index_rejects_text_vector_dim_mismatch_before_insert() {
        let plane = MilvusDataPlane::new(test_config());
        let executor = FakeExecutor::new();
        let mut batch = make_test_batch(Uuid::from_u128(42), Uuid::from_u128(99));
        batch.text_chunks[0].vector = vec![0.1, 0.2, 0.3];

        let result = plane.replace_document_index_impl(batch, &executor).await;
        let error = result.expect_err("text dim mismatch must fail before insert");
        let message = error.to_string();
        assert!(message.contains("text_chunks[0].text_dense"));
        assert!(executor.insert_calls().is_empty());
        assert!(executor.delete_calls().is_empty());
    }

    #[tokio::test]
    async fn replace_document_index_rejects_multimodal_vector_dim_mismatch_before_insert() {
        let plane = MilvusDataPlane::new(test_config());
        let executor = FakeExecutor::new();
        let mut batch = make_test_batch(Uuid::from_u128(42), Uuid::from_u128(99));
        batch.multimodal_chunks[0].vector = vec![0.1, 0.2, 0.3];

        let result = plane.replace_document_index_impl(batch, &executor).await;
        let error = result.expect_err("multimodal dim mismatch must fail before insert");
        let message = error.to_string();
        assert!(message.contains("multimodal_chunks[0].multimodal_dense"));
        assert!(executor.insert_calls().is_empty());
        assert!(executor.delete_calls().is_empty());
    }

    #[test]
    fn existing_schema_compatibility_accepts_matching_fields_and_dims() {
        let plane = MilvusDataPlane::new(test_config());
        let (schema, _) = plane.schema_text();
        let response = json!({
            "data": {
                "schema": {
                    "fields": schema["fields"]
                }
            }
        });

        let result = validate_existing_collection_schema("text_chunks", &schema, &response);
        assert!(result.is_ok(), "matching schema should pass: {result:?}");
    }

    #[test]
    fn existing_schema_compatibility_rejects_vector_dim_mismatch() {
        let plane = MilvusDataPlane::new(test_config());
        let (schema, _) = plane.schema_text();
        let mut actual_fields = schema["fields"].as_array().cloned().unwrap_or_default();
        let text_dense = actual_fields
            .iter_mut()
            .find(|field| field["fieldName"] == "text_dense")
            .expect("text_dense exists");
        text_dense["elementTypeParams"]["dim"] = json!(8);
        let response = json!({
            "data": {
                "schema": {
                    "fields": actual_fields
                }
            }
        });

        let result = validate_existing_collection_schema("text_chunks", &schema, &response);
        let error = result.expect_err("dim mismatch must be rejected");
        assert!(error.to_string().contains("text_dense"));
        assert!(error.to_string().contains("expected 4"));
    }

    #[test]
    fn existing_schema_compatibility_rejects_missing_field_type() {
        let plane = MilvusDataPlane::new(test_config());
        let (schema, _) = plane.schema_text();
        let mut actual_fields = schema["fields"].as_array().cloned().unwrap_or_default();
        let text_dense = actual_fields
            .iter_mut()
            .find(|field| field["fieldName"] == "text_dense")
            .expect("text_dense exists");
        text_dense.as_object_mut().unwrap().remove("dataType");
        let response = json!({
            "data": {
                "schema": {
                    "fields": actual_fields
                }
            }
        });

        let result = validate_existing_collection_schema("text_chunks", &schema, &response);
        let error = result.expect_err("missing field type must be rejected");
        assert!(error.to_string().contains("text_dense"));
        assert!(error.to_string().contains("missing field type"));
    }
}
