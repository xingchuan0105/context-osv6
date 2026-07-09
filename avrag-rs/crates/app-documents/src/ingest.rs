use app_core::{DocumentTaskSeed, StorageContext, StoredDocument, current_user_id};
use contracts::auth_runtime::AuthContext;
use common::AppError;
use contracts::documents::DocumentStatus;
use ingestion::{
    AuditAction, IngestDocumentPayload, ReindexDocumentPayload, ReindexReason, build_ingest_task,
    build_reindex_task, task_audit,
};

use crate::document_context::DocumentContext;

impl DocumentContext {
    pub async fn list_ready_documents_for_chat(
        &self,
        storage: &StorageContext,
        workspace_id: &str,
        doc_scope: &[String],
    ) -> Vec<StoredDocument> {
        let state = storage.inner().read().await;
        state
            .documents
            .values()
            .filter(|stored| stored.document.workspace_id == workspace_id)
            .filter(|stored| matches!(stored.document.status, DocumentStatus::Completed))
            .filter(|stored| doc_scope.is_empty() || doc_scope.contains(&stored.document.id))
            .cloned()
            .collect()
    }

    pub async fn enqueue_ingest_task(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        seed: DocumentTaskSeed,
    ) -> Result<(), AppError> {
        let Some(store) = storage.document_store() else {
            return Ok(());
        };

        let task = build_ingest_task(
            seed.org_id.clone(),
            seed.workspace_id.clone(),
            seed.document_id.clone(),
            Some(current_user_id(auth)),
            IngestDocumentPayload {
                source_uri: format!("object://{}", seed.object_path),
                object_path: seed.object_path.clone(),
                mime_type: seed.mime_type,
                filename: seed.filename,
                file_size: seed.file_size,
            },
        );
        let inserted = store.enqueue_ingestion_task(&task).await?;
        if inserted {
            store
                .append_audit_record(&task_audit(
                    &task,
                    AuditAction::TaskEnqueued,
                    serde_json::json!({
                        "kind": task.kind,
                        "document_id": task.document_id,
                        "object_path": match &task.payload {
                            ingestion::IngestionTaskPayload::IngestDocument(payload) => payload.object_path.clone(),
                            ingestion::IngestionTaskPayload::IngestUrl(payload) => payload.url.clone(),
                            ingestion::IngestionTaskPayload::ReindexDocument(_) => String::new(),
                        }
                    }),
                ))
                .await?;
        }
        Ok(())
    }

    pub async fn enqueue_reindex_task(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        seed: DocumentTaskSeed,
    ) -> Result<(), AppError> {
        let Some(store) = storage.document_store() else {
            return Ok(());
        };

        let task = build_reindex_task(
            seed.org_id,
            seed.workspace_id,
            seed.document_id,
            Some(current_user_id(auth)),
            ReindexDocumentPayload {
                reason: ReindexReason::Manual,
                requested_revision: (uuid::Uuid::new_v4().as_u128() & u32::MAX as u128) as u32,
            },
        );
        let inserted = store.enqueue_ingestion_task(&task).await?;
        if inserted {
            store
                .append_audit_record(&task_audit(
                    &task,
                    AuditAction::TaskEnqueued,
                    serde_json::json!({
                        "kind": task.kind,
                        "document_id": task.document_id,
                        "reason": "manual",
                    }),
                ))
                .await?;
        }
        Ok(())
    }
}
