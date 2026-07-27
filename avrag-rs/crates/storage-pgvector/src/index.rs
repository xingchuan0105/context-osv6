use crate::{PgvectorDataPlane, validate_vector_dim};
use pgvector::Vector;
use avrag_retrieval_data_plane::{DocumentIndexBatch, IndexWriteReport};
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

impl PgvectorDataPlane {
    pub(crate) async fn replace_document_index_impl(
        &self,
        batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport> {
        validate_batch(&batch, &self.config)?;

        let owner = batch.owner_user_id.into_uuid();
        let doc_id = batch.document_id;
        let workspace_id = batch.workspace_id;
        let parse_run_id = batch.parse_run_id;
        let doc_version = batch.doc_version as i32;

        let text_count = batch.text_chunks.len();
        let multimodal_count = batch.multimodal_chunks.len();
        let entity_count = batch.entities.len();
        let relation_count = batch.relations.len();
        let graph_passage_count = batch.graph_passages.len();

        let mut tx = self.pool.begin().await?;

        // Phase 0: purge this document across all rag tables.
        for table in [
            "rag_text_chunks",
            "rag_multimodal_chunks",
            "rag_kg_entities",
            "rag_kg_relations",
            "rag_graph_passages",
        ] {
            let sql = format!("DELETE FROM {table} WHERE owner_user_id = $1 AND doc_id = $2");
            sqlx::query(&sql)
                .bind(owner)
                .bind(doc_id)
                .execute(&mut *tx)
                .await?;
        }

        // Phase 1: insert.
        for chunk in &batch.text_chunks {
            let dense = Vector::from(chunk.vector.clone());
            sqlx::query(
                r#"
                INSERT INTO rag_text_chunks (
                    id, owner_user_id, workspace_id, doc_id, chunk_id, parse_run_id,
                    doc_version, page, text, text_dense, chunk_type, parser_backend, source_locator
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11, $12, $13
                )
                "#,
            )
            .bind(chunk.chunk_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(chunk.chunk_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(chunk.page)
            .bind(&chunk.content)
            .bind(&dense)
            .bind(&chunk.chunk_type)
            .bind(&chunk.parser_backend)
            .bind(&chunk.source_locator)
            .execute(&mut *tx)
            .await?;
        }

        for chunk in &batch.multimodal_chunks {
            let dense = Vector::from(chunk.vector.clone());
            sqlx::query(
                r#"
                INSERT INTO rag_multimodal_chunks (
                    id, owner_user_id, workspace_id, doc_id, chunk_id, asset_id, parse_run_id,
                    doc_version, page, context_text, caption, image_path, multimodal_dense,
                    chunk_type, parser_backend, retrieval_weight, source_locator
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17
                )
                "#,
            )
            .bind(chunk.chunk_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(chunk.chunk_id)
            .bind(chunk.asset_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(chunk.page)
            .bind(&chunk.context_text)
            .bind(&chunk.caption)
            .bind(&chunk.image_path)
            .bind(&dense)
            .bind(&chunk.chunk_type)
            .bind(&chunk.parser_backend)
            .bind(chunk.retrieval_weight)
            .bind(&chunk.source_locator)
            .execute(&mut *tx)
            .await?;
        }

        for entity in &batch.entities {
            let dense = Vector::from(entity.vector.clone());
            sqlx::query(
                r#"
                INSERT INTO rag_kg_entities (
                    id, owner_user_id, workspace_id, doc_id, entity_id, parse_run_id,
                    doc_version, name, normalized_name, entity_type, entity_dense,
                    supporting_chunk_ids, metadata
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11,
                    $12, $13
                )
                "#,
            )
            .bind(entity.entity_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(entity.entity_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(&entity.name)
            .bind(&entity.normalized_name)
            .bind(&entity.entity_type)
            .bind(&dense)
            .bind(&entity.supporting_chunk_ids)
            .bind(&entity.metadata)
            .execute(&mut *tx)
            .await?;
        }

        for relation in &batch.relations {
            let dense = Vector::from(relation.vector.clone());
            sqlx::query(
                r#"
                INSERT INTO rag_kg_relations (
                    id, owner_user_id, workspace_id, doc_id, relation_id, parse_run_id,
                    doc_version, subject, predicate, object, relation_text, relation_dense,
                    supporting_chunk_ids, metadata
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11, $12,
                    $13, $14
                )
                "#,
            )
            .bind(relation.relation_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(relation.relation_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(&relation.subject)
            .bind(&relation.predicate)
            .bind(&relation.object)
            .bind(&relation.relation_text)
            .bind(&dense)
            .bind(&relation.supporting_chunk_ids)
            .bind(&relation.metadata)
            .execute(&mut *tx)
            .await?;
        }

        for passage in &batch.graph_passages {
            let dense = Vector::from(passage.vector.clone());
            sqlx::query(
                r#"
                INSERT INTO rag_graph_passages (
                    id, owner_user_id, workspace_id, doc_id, chunk_id, passage_id, parse_run_id,
                    doc_version, text, passage_dense, relation_ids, metadata
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12
                )
                "#,
            )
            .bind(passage.passage_id.to_string())
            .bind(owner)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(passage.chunk_id)
            .bind(passage.passage_id)
            .bind(parse_run_id)
            .bind(doc_version)
            .bind(&passage.text)
            .bind(&dense)
            .bind(&passage.relation_ids)
            .bind(&passage.metadata)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(IndexWriteReport {
            text_chunk_count: text_count,
            multimodal_chunk_count: multimodal_count,
            entity_count,
            relation_count,
            graph_passage_count,
        })
    }

    pub(crate) async fn delete_document_index_impl(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> anyhow::Result<()> {
        let owner = *auth.user_id().uuid();
        let mut tx = self.pool.begin().await?;
        for table in [
            "rag_text_chunks",
            "rag_multimodal_chunks",
            "rag_kg_entities",
            "rag_kg_relations",
            "rag_graph_passages",
        ] {
            let sql = format!("DELETE FROM {table} WHERE owner_user_id = $1 AND doc_id = $2");
            sqlx::query(&sql)
                .bind(owner)
                .bind(document_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn validate_batch(
    batch: &DocumentIndexBatch,
    config: &crate::PgvectorConfig,
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
