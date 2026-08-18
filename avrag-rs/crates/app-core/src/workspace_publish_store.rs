//! Local → cloud workspace publish mapping (ADR-0010 B3b).

use std::collections::BTreeMap;

use async_trait::async_trait;
use avrag_retrieval_data_plane::{DocumentIndexExport, PublishFingerprint};
use chrono::{DateTime, Utc};
use common::AppError;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PublishStatus {
    Never,
    Publishing,
    Ready,
    Failed,
}

impl PublishStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Publishing => "publishing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw {
            "publishing" => Self::Publishing,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::Never,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePublishRow {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub cloud_workspace_id: Uuid,
    pub local_workspace_id: Uuid,
    pub upload_id: Option<Uuid>,
    pub status: PublishStatus,
    pub embedding_model_id: String,
    pub vector_dim: i32,
    pub expected_parts: i32,
    pub last_published_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePublishSessionRequest {
    pub local_workspace_id: Uuid,
    pub title: String,
    pub fingerprint: PublishFingerprint,
    pub document_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePublishSessionResponse {
    pub upload_id: Uuid,
    pub cloud_workspace_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPartPayload {
    pub document_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub status: String,
    pub summary: Option<String>,
    pub chunk_count: usize,
    pub export: DocumentIndexExport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishStatusResponse {
    pub status: PublishStatus,
    pub cloud_workspace_id: Option<Uuid>,
    pub last_published_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub expected_parts: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishExportListResponse {
    pub fingerprint: PublishFingerprint,
    pub document_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct PublishedDocumentUpsert {
    pub document_id: Uuid,
    pub workspace_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub summary: Option<String>,
    pub chunk_count: usize,
}

#[async_trait]
pub trait WorkspacePublishStorePort: Send + Sync {
    async fn get_by_local(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError>;

    async fn get_by_upload(
        &self,
        owner_user_id: Uuid,
        upload_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError>;

    async fn upsert_session(&self, row: &WorkspacePublishRow) -> Result<WorkspacePublishRow, AppError>;

    async fn mark_status(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
        status: PublishStatus,
        error: Option<&str>,
        last_published_at: Option<DateTime<Utc>>,
    ) -> Result<WorkspacePublishRow, AppError>;
}

#[derive(Default)]
pub struct MemoryWorkspacePublishStore {
    rows: RwLock<BTreeMap<(Uuid, Uuid), WorkspacePublishRow>>,
}

impl MemoryWorkspacePublishStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkspacePublishStorePort for MemoryWorkspacePublishStore {
    async fn get_by_local(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError> {
        Ok(self
            .rows
            .read()
            .await
            .get(&(owner_user_id, local_workspace_id))
            .cloned())
    }

    async fn get_by_upload(
        &self,
        owner_user_id: Uuid,
        upload_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError> {
        Ok(self
            .rows
            .read()
            .await
            .values()
            .find(|row| row.owner_user_id == owner_user_id && row.upload_id == Some(upload_id))
            .cloned())
    }

    async fn upsert_session(
        &self,
        row: &WorkspacePublishRow,
    ) -> Result<WorkspacePublishRow, AppError> {
        self.rows
            .write()
            .await
            .insert((row.owner_user_id, row.local_workspace_id), row.clone());
        Ok(row.clone())
    }

    async fn mark_status(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
        status: PublishStatus,
        error: Option<&str>,
        last_published_at: Option<DateTime<Utc>>,
    ) -> Result<WorkspacePublishRow, AppError> {
        let mut rows = self.rows.write().await;
        let row = rows
            .get_mut(&(owner_user_id, local_workspace_id))
            .ok_or_else(|| {
                AppError::not_found("publish_mapping_not_found", "publish mapping not found")
            })?;
        row.status = status;
        row.error = error.map(str::to_string);
        if last_published_at.is_some() {
            row.last_published_at = last_published_at;
        }
        Ok(row.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_roundtrip() {
        let store = MemoryWorkspacePublishStore::new();
        let owner = Uuid::from_u128(1);
        let local = Uuid::from_u128(2);
        let upload = Uuid::from_u128(3);
        let row = WorkspacePublishRow {
            id: Uuid::from_u128(4),
            owner_user_id: owner,
            cloud_workspace_id: Uuid::from_u128(5),
            local_workspace_id: local,
            upload_id: Some(upload),
            status: PublishStatus::Publishing,
            embedding_model_id: "text-embedding-v4".into(),
            vector_dim: 1024,
            expected_parts: 1,
            last_published_at: None,
            error: None,
        };
        store.upsert_session(&row).await.unwrap();
        assert_eq!(
            store.get_by_local(owner, local).await.unwrap().unwrap().upload_id,
            Some(upload)
        );
        store
            .mark_status(owner, local, PublishStatus::Ready, None, Some(Utc::now()))
            .await
            .unwrap();
        assert_eq!(
            store.get_by_upload(owner, upload).await.unwrap().unwrap().status,
            PublishStatus::Ready
        );
    }
}
