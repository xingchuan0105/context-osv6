//! Export full index records including embedding vectors (ADR-0010 PR11).

use crate::{owner_uuid, PgvectorDataPlane};
use avrag_retrieval_data_plane::{
    DocumentExportManifest, DocumentIndexBatch, DocumentIndexExport, EntityIndexRecord,
    ExportDocumentRequest, GraphPassageIndexRecord, MultimodalChunkIndexRecord,
    PublishFingerprint, RelationIndexRecord, TextChunkIndexRecord,
};
use contracts::auth_runtime::{AuthContext, UserId};
use pgvector::Vector;
use serde_json::Value;
use uuid::Uuid;

impl PgvectorDataPlane {
    pub(crate) async fn export_document_index_impl(
        &self,
        request: ExportDocumentRequest,
    ) -> anyhow::Result<DocumentIndexExport> {
        let owner = owner_uuid(&request.auth);
        let doc_id = request.document_id;
        let workspace_id = request.workspace_id;

        let text_chunks =
            self.load_text_chunk_records(owner, doc_id, workspace_id).await?;
        let multimodal_chunks =
            self.load_multimodal_chunk_records(owner, doc_id, workspace_id)
                .await?;
        let entities = self
            .load_entity_records(owner, doc_id, workspace_id)
            .await?;
        let relations = self
            .load_relation_records(owner, doc_id, workspace_id)
            .await?;
        let graph_passages = self
            .load_graph_passage_records(owner, doc_id, workspace_id)
            .await?;

        let (parse_run_id, doc_version, resolved_workspace) = resolve_batch_identity(
            &text_chunks,
            &multimodal_chunks,
            &entities,
            &relations,
            &graph_passages,
            workspace_id,
        );

        let batch = DocumentIndexBatch {
            owner_user_id: UserId::from(owner),
            workspace_id: resolved_workspace,
            document_id: doc_id,
            parse_run_id,
            doc_version,
            text_chunks: text_chunks.into_iter().map(|r| r.record).collect(),
            multimodal_chunks: multimodal_chunks.into_iter().map(|r| r.record).collect(),
            entities: entities.into_iter().map(|r| r.record).collect(),
            relations: relations.into_iter().map(|r| r.record).collect(),
            graph_passages: graph_passages.into_iter().map(|r| r.record).collect(),
        };

        // Validate exported vectors against caller fingerprint (and local config dims).
        request.fingerprint.validate_batch(&batch)?;
        validate_against_config(&batch, &self.config, &request.fingerprint)?;

        let manifest = DocumentExportManifest {
            fingerprint: request.fingerprint,
            owner_user_id: batch.owner_user_id,
            workspace_id: batch.workspace_id,
            document_id: batch.document_id,
            parse_run_id: batch.parse_run_id,
            doc_version: batch.doc_version,
            text_chunk_count: batch.text_chunks.len(),
            multimodal_chunk_count: batch.multimodal_chunks.len(),
            entity_count: batch.entities.len(),
            relation_count: batch.relations.len(),
            graph_passage_count: batch.graph_passages.len(),
        };

        Ok(DocumentIndexExport { manifest, batch })
    }

    pub(crate) async fn list_indexed_document_ids_impl(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<Uuid>> {
        let owner = owner_uuid(auth);
        // Union doc_ids across all rag_* tables so graph-only docs still appear.
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, (Uuid,)>(
                r#"
                SELECT DISTINCT doc_id FROM (
                    SELECT doc_id FROM rag_text_chunks
                      WHERE owner_user_id = $1 AND workspace_id = $2
                    UNION
                    SELECT doc_id FROM rag_multimodal_chunks
                      WHERE owner_user_id = $1 AND workspace_id = $2
                    UNION
                    SELECT doc_id FROM rag_kg_entities
                      WHERE owner_user_id = $1 AND workspace_id = $2
                    UNION
                    SELECT doc_id FROM rag_kg_relations
                      WHERE owner_user_id = $1 AND workspace_id = $2
                    UNION
                    SELECT doc_id FROM rag_graph_passages
                      WHERE owner_user_id = $1 AND workspace_id = $2
                ) AS docs
                ORDER BY doc_id
                "#,
            )
            .bind(owner)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (Uuid,)>(
                r#"
                SELECT DISTINCT doc_id FROM (
                    SELECT doc_id FROM rag_text_chunks WHERE owner_user_id = $1
                    UNION
                    SELECT doc_id FROM rag_multimodal_chunks WHERE owner_user_id = $1
                    UNION
                    SELECT doc_id FROM rag_kg_entities WHERE owner_user_id = $1
                    UNION
                    SELECT doc_id FROM rag_kg_relations WHERE owner_user_id = $1
                    UNION
                    SELECT doc_id FROM rag_graph_passages WHERE owner_user_id = $1
                ) AS docs
                ORDER BY doc_id
                "#,
            )
            .bind(owner)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn load_text_chunk_records(
        &self,
        owner: Uuid,
        doc_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<RowWithMeta<TextChunkIndexRecord>>> {
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, TextExportRow>(
                r#"
                SELECT chunk_id, text, text_dense, page, chunk_type, parser_backend,
                       source_locator, parse_run_id, doc_version, workspace_id
                FROM rag_text_chunks
                WHERE owner_user_id = $1 AND doc_id = $2 AND workspace_id = $3
                ORDER BY chunk_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TextExportRow>(
                r#"
                SELECT chunk_id, text, text_dense, page, chunk_type, parser_backend,
                       source_locator, parse_run_id, doc_version, workspace_id
                FROM rag_text_chunks
                WHERE owner_user_id = $1 AND doc_id = $2
                ORDER BY chunk_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| RowWithMeta {
                parse_run_id: r.parse_run_id,
                doc_version: r.doc_version,
                workspace_id: r.workspace_id,
                record: TextChunkIndexRecord {
                    chunk_id: r.chunk_id,
                    content: r.text,
                    vector: r.text_dense.to_vec(),
                    page: r.page,
                    chunk_type: r.chunk_type,
                    parser_backend: r.parser_backend,
                    source_locator: r.source_locator,
                },
            })
            .collect())
    }

    async fn load_multimodal_chunk_records(
        &self,
        owner: Uuid,
        doc_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<RowWithMeta<MultimodalChunkIndexRecord>>> {
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, MultimodalExportRow>(
                r#"
                SELECT chunk_id, asset_id, context_text, caption, image_path,
                       multimodal_dense, page, chunk_type, parser_backend,
                       source_locator, retrieval_weight, parse_run_id, doc_version, workspace_id
                FROM rag_multimodal_chunks
                WHERE owner_user_id = $1 AND doc_id = $2 AND workspace_id = $3
                ORDER BY chunk_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MultimodalExportRow>(
                r#"
                SELECT chunk_id, asset_id, context_text, caption, image_path,
                       multimodal_dense, page, chunk_type, parser_backend,
                       source_locator, retrieval_weight, parse_run_id, doc_version, workspace_id
                FROM rag_multimodal_chunks
                WHERE owner_user_id = $1 AND doc_id = $2
                ORDER BY chunk_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| RowWithMeta {
                parse_run_id: r.parse_run_id,
                doc_version: r.doc_version,
                workspace_id: r.workspace_id,
                record: MultimodalChunkIndexRecord {
                    chunk_id: r.chunk_id,
                    asset_id: r.asset_id,
                    context_text: r.context_text,
                    caption: r.caption,
                    image_path: r.image_path,
                    vector: r.multimodal_dense.to_vec(),
                    page: r.page,
                    chunk_type: r.chunk_type,
                    parser_backend: r.parser_backend,
                    source_locator: r.source_locator,
                    retrieval_weight: r.retrieval_weight,
                },
            })
            .collect())
    }

    async fn load_entity_records(
        &self,
        owner: Uuid,
        doc_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<RowWithMeta<EntityIndexRecord>>> {
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, EntityExportRow>(
                r#"
                SELECT entity_id, name, normalized_name, entity_type, entity_dense,
                       supporting_chunk_ids, metadata, parse_run_id, doc_version, workspace_id
                FROM rag_kg_entities
                WHERE owner_user_id = $1 AND doc_id = $2 AND workspace_id = $3
                ORDER BY entity_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, EntityExportRow>(
                r#"
                SELECT entity_id, name, normalized_name, entity_type, entity_dense,
                       supporting_chunk_ids, metadata, parse_run_id, doc_version, workspace_id
                FROM rag_kg_entities
                WHERE owner_user_id = $1 AND doc_id = $2
                ORDER BY entity_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| RowWithMeta {
                parse_run_id: r.parse_run_id,
                doc_version: r.doc_version,
                workspace_id: r.workspace_id,
                record: EntityIndexRecord {
                    entity_id: r.entity_id,
                    name: r.name,
                    normalized_name: r.normalized_name,
                    entity_type: r.entity_type,
                    vector: r.entity_dense.to_vec(),
                    supporting_chunk_ids: r.supporting_chunk_ids,
                    metadata: r.metadata,
                },
            })
            .collect())
    }

    async fn load_relation_records(
        &self,
        owner: Uuid,
        doc_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<RowWithMeta<RelationIndexRecord>>> {
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, RelationExportRow>(
                r#"
                SELECT relation_id, subject, predicate, object, relation_text, relation_dense,
                       supporting_chunk_ids, metadata, parse_run_id, doc_version, workspace_id
                FROM rag_kg_relations
                WHERE owner_user_id = $1 AND doc_id = $2 AND workspace_id = $3
                ORDER BY relation_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, RelationExportRow>(
                r#"
                SELECT relation_id, subject, predicate, object, relation_text, relation_dense,
                       supporting_chunk_ids, metadata, parse_run_id, doc_version, workspace_id
                FROM rag_kg_relations
                WHERE owner_user_id = $1 AND doc_id = $2
                ORDER BY relation_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| RowWithMeta {
                parse_run_id: r.parse_run_id,
                doc_version: r.doc_version,
                workspace_id: r.workspace_id,
                record: RelationIndexRecord {
                    relation_id: r.relation_id,
                    subject: r.subject,
                    predicate: r.predicate,
                    object: r.object,
                    relation_text: r.relation_text,
                    vector: r.relation_dense.to_vec(),
                    supporting_chunk_ids: r.supporting_chunk_ids,
                    metadata: r.metadata,
                },
            })
            .collect())
    }

    async fn load_graph_passage_records(
        &self,
        owner: Uuid,
        doc_id: Uuid,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<RowWithMeta<GraphPassageIndexRecord>>> {
        let rows = if let Some(ws) = workspace_id {
            sqlx::query_as::<_, PassageExportRow>(
                r#"
                SELECT passage_id, chunk_id, text, passage_dense, relation_ids, metadata,
                       parse_run_id, doc_version, workspace_id
                FROM rag_graph_passages
                WHERE owner_user_id = $1 AND doc_id = $2 AND workspace_id = $3
                ORDER BY passage_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .bind(ws)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, PassageExportRow>(
                r#"
                SELECT passage_id, chunk_id, text, passage_dense, relation_ids, metadata,
                       parse_run_id, doc_version, workspace_id
                FROM rag_graph_passages
                WHERE owner_user_id = $1 AND doc_id = $2
                ORDER BY passage_id
                "#,
            )
            .bind(owner)
            .bind(doc_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|r| RowWithMeta {
                parse_run_id: r.parse_run_id,
                doc_version: r.doc_version,
                workspace_id: r.workspace_id,
                record: GraphPassageIndexRecord {
                    passage_id: r.passage_id,
                    chunk_id: r.chunk_id,
                    text: r.text,
                    vector: r.passage_dense.to_vec(),
                    relation_ids: r.relation_ids,
                    metadata: r.metadata,
                },
            })
            .collect())
    }
}

struct RowWithMeta<T> {
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
    record: T,
}

fn resolve_batch_identity<A, B, C, D, E>(
    text: &[RowWithMeta<A>],
    multi: &[RowWithMeta<B>],
    entities: &[RowWithMeta<C>],
    relations: &[RowWithMeta<D>],
    passages: &[RowWithMeta<E>],
    request_workspace: Option<Uuid>,
) -> (Uuid, u32, Option<Uuid>) {
    // Prefer any row's identity; replace_document_index writes them consistently.
    let meta = text
        .first()
        .map(|r| (r.parse_run_id, r.doc_version, r.workspace_id))
        .or_else(|| {
            multi
                .first()
                .map(|r| (r.parse_run_id, r.doc_version, r.workspace_id))
        })
        .or_else(|| {
            entities
                .first()
                .map(|r| (r.parse_run_id, r.doc_version, r.workspace_id))
        })
        .or_else(|| {
            relations
                .first()
                .map(|r| (r.parse_run_id, r.doc_version, r.workspace_id))
        })
        .or_else(|| {
            passages
                .first()
                .map(|r| (r.parse_run_id, r.doc_version, r.workspace_id))
        });

    match meta {
        Some((parse_run_id, doc_version, workspace_id)) => (
            parse_run_id,
            doc_version.max(0) as u32,
            workspace_id.or(request_workspace),
        ),
        None => (Uuid::nil(), 0, request_workspace),
    }
}

fn validate_against_config(
    batch: &DocumentIndexBatch,
    config: &crate::PgvectorConfig,
    fingerprint: &PublishFingerprint,
) -> anyhow::Result<()> {
    // Config dims must match fingerprint so re-import on this node stays consistent.
    if fingerprint.vector_dim != config.text_vector_dim {
        anyhow::bail!(
            "fingerprint vector_dim {} does not match data-plane text_vector_dim {}",
            fingerprint.vector_dim,
            config.text_vector_dim
        );
    }
    if fingerprint.multimodal_dim() != config.multimodal_vector_dim {
        anyhow::bail!(
            "fingerprint multimodal_vector_dim {} does not match data-plane multimodal_vector_dim {}",
            fingerprint.multimodal_dim(),
            config.multimodal_vector_dim
        );
    }
    // Empty batch is valid (no vectors to check further).
    let _ = batch;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct TextExportRow {
    chunk_id: Uuid,
    text: String,
    text_dense: Vector,
    page: Option<i64>,
    chunk_type: String,
    parser_backend: Option<String>,
    source_locator: Option<Value>,
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct MultimodalExportRow {
    chunk_id: Uuid,
    asset_id: Uuid,
    context_text: String,
    caption: Option<String>,
    image_path: Option<String>,
    multimodal_dense: Vector,
    page: Option<i64>,
    chunk_type: String,
    parser_backend: Option<String>,
    source_locator: Option<Value>,
    retrieval_weight: Option<f32>,
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct EntityExportRow {
    entity_id: Uuid,
    name: String,
    normalized_name: String,
    entity_type: Option<String>,
    entity_dense: Vector,
    supporting_chunk_ids: Vec<Uuid>,
    metadata: Option<Value>,
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct RelationExportRow {
    relation_id: Uuid,
    subject: String,
    predicate: String,
    object: String,
    relation_text: String,
    relation_dense: Vector,
    supporting_chunk_ids: Vec<Uuid>,
    metadata: Option<Value>,
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct PassageExportRow {
    passage_id: Uuid,
    chunk_id: Option<Uuid>,
    text: String,
    passage_dense: Vector,
    relation_ids: Vec<Uuid>,
    metadata: Option<Value>,
    parse_run_id: Uuid,
    doc_version: i32,
    workspace_id: Option<Uuid>,
}
