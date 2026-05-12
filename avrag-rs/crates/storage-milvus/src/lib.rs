use async_trait::async_trait;
use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, DocumentIndexBatch, GraphSearchOutput,
    GraphSearchRequest, IndexWriteReport, MultimodalSearchRequest,
    RetrievalDataPlane, ScoredChunk, TextDenseSearchRequest,
};

pub mod types;
pub mod config;
pub mod schema;
pub mod executor;
pub mod lib_impl;
pub mod utils;
pub mod ops;

pub use config::{MilvusConfig, TenantContext, MilvusCollectionNames};
pub use types::{MilvusStorageError, Result};
pub use lib_impl::MilvusDataPlane;

#[async_trait]
impl RetrievalDataPlane for MilvusDataPlane {
    async fn ensure_schema(&self) -> anyhow::Result<()> {
        let names = self.config.collection_names();
        let existing = self.list_collections().await?;

        let (schema, indexes) = schema::schema_text(&self.config);
        self.create_collection_if_missing(&existing, &names.text_chunks, schema, indexes)
            .await?;

        let (schema, indexes) = schema::schema_multimodal(&self.config);
        self.create_collection_if_missing(&existing, &names.multimodal_chunks, schema, indexes)
            .await?;

        let (schema, indexes) = schema::schema_entities(&self.config);
        self.create_collection_if_missing(&existing, &names.kg_entities, schema, indexes)
            .await?;

        let (schema, indexes) = schema::schema_relations(&self.config);
        self.create_collection_if_missing(&existing, &names.kg_relations, schema, indexes)
            .await?;

        let (schema, indexes) = schema::schema_graph_passages(&self.config);
        self.create_collection_if_missing(&existing, &names.graph_passages, schema, indexes)
            .await?;

        Ok(())
    }

    async fn delete_document_index(&self, auth: &AuthContext, document_id: uuid::Uuid) -> anyhow::Result<()> {
        let names = self.config.collection_names();
        let filter = schema::doc_filter(auth, Some(&[document_id]));
        for collection in [
            names.text_chunks,
            names.multimodal_chunks,
            names.kg_entities,
            names.kg_relations,
            names.graph_passages,
        ] {
            let _ = self.delete_by_filter(&collection, filter.clone()).await;
        }
        Ok(())
    }

    async fn replace_document_index(&self, batch: DocumentIndexBatch) -> anyhow::Result<IndexWriteReport> {
        let executor = executor::RealExecutor { plane: self };
        self.replace_document_index_impl(batch, &executor).await
    }

    async fn search_text_dense(&self, request: TextDenseSearchRequest) -> anyhow::Result<Vec<ScoredChunk>> {
        self.search_text_dense(request).await
    }

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        self.search_bm25(request).await
    }

    async fn search_multimodal(&self, request: MultimodalSearchRequest) -> anyhow::Result<Vec<ScoredChunk>> {
        self.search_multimodal(request).await
    }

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        self.search_graph(request).await
    }
}

impl MilvusDataPlane {
    async fn create_collection_if_missing(
        &self,
        existing_collections: &[String],
        collection_name: &str,
        schema: serde_json::Value,
        indexes: Vec<serde_json::Value>,
    ) -> Result<()> {
        if !existing_collections.contains(&collection_name.to_string()) {
            self.post_json(
                "/v2/vectordb/collections/create",
                self.with_database(serde_json::json!({
                    "collectionName": collection_name,
                    "schema": schema,
                    "indexParams": indexes
                })),
            )
            .await?;
        } else {
            // Validate compatibility
            let response = self
                .post_json(
                    "/v2/vectordb/collections/describe",
                    self.with_database(serde_json::json!({
                        "collectionName": collection_name
                    })),
                )
                .await?;
            schema::validate_existing_collection_schema(collection_name, &schema, &response)?;
        }
        Ok(())
    }
}
