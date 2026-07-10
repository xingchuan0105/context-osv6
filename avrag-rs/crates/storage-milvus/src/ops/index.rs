use crate::config::MilvusConfig;
use crate::executor::WriteExecutor;
use crate::lib_impl::MilvusDataPlane;
use crate::types::{MilvusStorageError, Result};
use avrag_retrieval_data_plane::{DocumentIndexBatch, IndexWriteReport};
use serde_json::{Value, json};
use uuid::Uuid;

impl MilvusDataPlane {
    pub(crate) async fn insert_if_nonempty<E: WriteExecutor>(
        &self,
        executor: &E,
        collection: &str,
        rows: Vec<Value>,
        attempted: &mut Vec<String>,
    ) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        attempted.push(collection.to_string());
        executor.insert(collection, rows).await
    }

    pub(crate) async fn cleanup_current_parse_run<E: WriteExecutor>(
        &self,
        executor: &E,
        collections: &[String],
        document_id: &Uuid,
        parse_run_id: &Uuid,
    ) -> Result<()> {
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

    pub(crate) async fn replace_document_index_impl<E: WriteExecutor>(
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

        // Phase 0: Pre-cleanup.
        let purge_filter = format!(
            "owner_user_id == '{}' && doc_id == '{}'",
            batch.owner_user_id, batch.document_id
        );
        let collections = [
            &names.text_chunks,
            &names.multimodal_chunks,
            &names.kg_entities,
            &names.kg_relations,
            &names.graph_passages,
        ];

        for collection in collections {
            let _ = executor.delete(collection, purge_filter.clone()).await;
        }

        // Phase 1: Insert new data.
        let mut attempted: Vec<String> = Vec::new();

        let text_rows: Vec<Value> = if !batch.text_chunks.is_empty() {
            batch
                .text_chunks
                .iter()
                .map(|chunk| {
                    json!({
                        "id": chunk.chunk_id.to_string(),
                        "owner_user_id": batch.owner_user_id.to_string(),
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
                        "owner_user_id": batch.owner_user_id.to_string(),
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
                        "retrieval_weight": chunk.retrieval_weight,
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
                        "owner_user_id": batch.owner_user_id.to_string(),
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
                        "owner_user_id": batch.owner_user_id.to_string(),
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
                        "owner_user_id": batch.owner_user_id.to_string(),
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

        Ok(IndexWriteReport {
            text_chunk_count: text_count,
            multimodal_chunk_count: multimodal_count,
            entity_count,
            relation_count,
            graph_passage_count,
        })
    }
}

pub(crate) async fn cleanup_partial_and_err<E: WriteExecutor>(
    plane: &MilvusDataPlane,
    executor: &E,
    insert_err: MilvusStorageError,
    attempted: &[String],
    document_id: &Uuid,
    parse_run_id: &Uuid,
) -> anyhow::Result<IndexWriteReport> {
    match plane
        .cleanup_current_parse_run(executor, attempted, document_id, parse_run_id)
        .await
    {
        Ok(_) => Err(anyhow::anyhow!(
            "insert failed: {}. Cleanup of partial writes successful for collections: {:?}",
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

pub(crate) fn validate_document_batch_vector_dims(
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
