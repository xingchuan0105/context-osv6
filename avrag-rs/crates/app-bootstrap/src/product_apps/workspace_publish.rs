//! Workspace publish (ADR-0010 B3b): local index export + cloud vector import.

use std::collections::HashSet;
use std::io::Read;
use std::sync::Arc;

use app_core::{
    CreatePublishSessionRequest, CreatePublishSessionResponse, ObjectStorePort,
    PublishExportListResponse, PublishPartPayload, PublishStatus, PublishStatusResponse,
    PublishedDocumentUpsert, WorkspacePublishRow, WorkspacePublishStorePort,
};
use avrag_retrieval_data_plane::{
    ExportDocumentRequest, PublishFingerprint, RetrievalDataPlane, RetrievalExportPort,
};
use chrono::Utc;
use common::AppError;
use uuid::Uuid;

use super::WorkspaceApp;

#[derive(Clone)]
pub struct WorkspacePublishRuntime {
    pub store: Arc<dyn WorkspacePublishStorePort>,
    pub retrieval_data_plane: Option<Arc<dyn RetrievalDataPlane>>,
    pub retrieval_export: Option<Arc<dyn RetrievalExportPort>>,
    pub fingerprint: PublishFingerprint,
}

impl WorkspacePublishRuntime {
    pub fn memory(fingerprint: PublishFingerprint) -> Self {
        Self {
            store: Arc::new(app_core::MemoryWorkspacePublishStore::new()),
            retrieval_data_plane: None,
            retrieval_export: None,
            fingerprint,
        }
    }
}

fn part_object_path(upload_id: Uuid, n: u32) -> String {
    format!("publish/{upload_id}/part-{n}.json")
}

const ZSTD_LEVEL: i32 = 3;
const ZSTD_MAX_DECODE: usize = 128 * 1024 * 1024;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

fn fingerprint_mismatch(field: &str) -> AppError {
    AppError::validation(
        "publish_fingerprint_mismatch",
        format!("embedding fingerprint mismatch ({field}); cloud will not re-embed"),
    )
}

fn payload_too_large() -> AppError {
    AppError::Validation {
        code: "publish_part_too_large",
        message: "decompressed publish part exceeds 128MiB".into(),
        http_status: 413,
    }
}

fn compress_part_json(json: &[u8]) -> Result<Vec<u8>, AppError> {
    zstd::encode_all(json, ZSTD_LEVEL)
        .map_err(|err| AppError::internal(format!("zstd compress publish part: {err}")))
}

fn looks_like_zstd(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[..4] == ZSTD_MAGIC
}

fn decompress_part_bytes(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut decoder = zstd::stream::read::Decoder::new(bytes).map_err(|err| {
        AppError::validation("publish_part_invalid", format!("zstd: {err}"))
    })?;
    let mut out = Vec::new();
    let mut buf = [0u8; 32 * 1024];
    loop {
        let n = decoder.read(&mut buf).map_err(|err| {
            AppError::validation("publish_part_invalid", format!("zstd: {err}"))
        })?;
        if n == 0 {
            break;
        }
        if out.len().saturating_add(n) > ZSTD_MAX_DECODE {
            return Err(payload_too_large());
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

fn json_from_part_bytes(bytes: &[u8], content_encoding: Option<&str>) -> Result<Vec<u8>, AppError> {
    let encoding = content_encoding.unwrap_or("").trim().to_ascii_lowercase();
    if encoding == "zstd" || looks_like_zstd(bytes) {
        decompress_part_bytes(bytes)
    } else {
        if bytes.len() > ZSTD_MAX_DECODE {
            return Err(payload_too_large());
        }
        Ok(bytes.to_vec())
    }
}

fn parse_part_payload(json: &[u8]) -> Result<PublishPartPayload, AppError> {
    serde_json::from_slice(json)
        .map_err(|err| AppError::validation("publish_part_invalid", err.to_string()))
}

impl WorkspaceApp<'_> {
    fn owner_uuid(&self) -> Uuid {
        self.auth.user_id().into_uuid()
    }

    fn object_store(&self) -> &Arc<dyn ObjectStorePort> {
        self.storage.object_store()
    }

    pub async fn create_publish_session(
        &self,
        req: CreatePublishSessionRequest,
    ) -> Result<CreatePublishSessionResponse, AppError> {
        if req.document_ids.is_empty() {
            return Err(AppError::validation(
                "publish_empty",
                "no indexed documents to publish",
            ));
        }
        if let Some(field) = req.fingerprint.incompatible_field(&self.publish.fingerprint) {
            return Err(fingerprint_mismatch(field));
        }

        let owner = self.owner_uuid();
        let existing = self
            .publish
            .store
            .get_by_local(owner, req.local_workspace_id)
            .await?;

        let cloud_workspace_id = if let Some(row) = existing.as_ref() {
            match self
                .docs
                .get_workspace(self.auth, self.storage, &row.cloud_workspace_id.to_string())
                .await
            {
                Some(ws) => Uuid::parse_str(&ws.id).unwrap_or(row.cloud_workspace_id),
                None => {
                    let created = self
                        .docs
                        .create_workspace(
                            self.auth,
                            self.storage,
                            self.analytics,
                            common::CreateWorkspaceRequest {
                                name: req.title.clone(),
                                description: String::new(),
                            },
                        )
                        .await?;
                    Uuid::parse_str(&created.id).map_err(|_| {
                        AppError::internal("created cloud workspace id is not a uuid")
                    })?
                }
            }
        } else {
            let created = self
                .docs
                .create_workspace(
                    self.auth,
                    self.storage,
                    self.analytics,
                    common::CreateWorkspaceRequest {
                        name: req.title.clone(),
                        description: String::new(),
                    },
                )
                .await?;
            Uuid::parse_str(&created.id)
                .map_err(|_| AppError::internal("created cloud workspace id is not a uuid"))?
        };

        let upload_id = Uuid::new_v4();
        let row = WorkspacePublishRow {
            id: existing
                .as_ref()
                .map(|row| row.id)
                .unwrap_or_else(Uuid::new_v4),
            owner_user_id: owner,
            cloud_workspace_id,
            local_workspace_id: req.local_workspace_id,
            upload_id: Some(upload_id),
            status: PublishStatus::Publishing,
            embedding_model_id: req.fingerprint.embedding_model_id.clone(),
            vector_dim: i32::try_from(req.fingerprint.vector_dim).unwrap_or(i32::MAX),
            expected_parts: i32::try_from(req.document_ids.len()).unwrap_or(i32::MAX),
            last_published_at: existing.and_then(|row| row.last_published_at),
            error: None,
        };
        self.publish.store.upsert_session(&row).await?;
        Ok(CreatePublishSessionResponse {
            upload_id,
            cloud_workspace_id,
        })
    }

    pub async fn put_publish_part(
        &self,
        upload_id: Uuid,
        part_n: u32,
        body: Vec<u8>,
        content_encoding: Option<&str>,
    ) -> Result<(), AppError> {
        let json = json_from_part_bytes(&body, content_encoding)?;
        let payload = parse_part_payload(&json)?;
        let owner = self.owner_uuid();
        let row = self
            .publish
            .store
            .get_by_upload(owner, upload_id)
            .await?
            .ok_or_else(|| AppError::not_found("publish_session_not_found", "publish session not found"))?;
        if row.status != PublishStatus::Publishing {
            return Err(AppError::conflict(
                "publish_session_not_open",
                "publish session is not accepting parts",
            ));
        }
        if part_n as i32 >= row.expected_parts {
            return Err(AppError::validation(
                "publish_part_out_of_range",
                format!("part {part_n} exceeds expected_parts {}", row.expected_parts),
            ));
        }
        if let Some(field) = payload
            .export
            .manifest
            .fingerprint
            .incompatible_field(&self.publish.fingerprint)
        {
            return Err(fingerprint_mismatch(field));
        }
        self.publish
            .fingerprint
            .validate_batch(&payload.export.batch)
            .map_err(|err| AppError::validation("publish_fingerprint_mismatch", err.to_string()))?;
        if payload.export.batch.document_id != payload.document_id {
            return Err(AppError::validation(
                "publish_document_id_mismatch",
                "part document_id does not match index batch",
            ));
        }
        let stored = if looks_like_zstd(&body) {
            body
        } else {
            compress_part_json(&json)?
        };
        self.object_store()
            .put(&part_object_path(upload_id, part_n), &stored)
            .await
    }

    pub async fn commit_publish_session(
        &self,
        upload_id: Uuid,
    ) -> Result<PublishStatusResponse, AppError> {
        let owner = self.owner_uuid();
        let row = self
            .publish
            .store
            .get_by_upload(owner, upload_id)
            .await?
            .ok_or_else(|| AppError::not_found("publish_session_not_found", "publish session not found"))?;
        match self.commit_publish_session_inner(&row).await {
            Ok(ready) => Ok(ready),
            Err(error) => {
                self.fail_publish_session(&row, error.message()).await;
                Err(error)
            }
        }
    }

    async fn commit_publish_session_inner(
        &self,
        row: &WorkspacePublishRow,
    ) -> Result<PublishStatusResponse, AppError> {
        let plane = self.publish.retrieval_data_plane.clone().ok_or_else(|| {
            AppError::upstream_unavailable("retrieval data plane is not available for publish import")
        })?;
        let docs = self.storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for publish commit")
        })?;
        let upload_id = row.upload_id.unwrap_or(row.id);

        let mut parts = Vec::new();
        for n in 0..row.expected_parts.max(0) as u32 {
            let bytes = self
                .object_store()
                .get(&part_object_path(upload_id, n))
                .await
                .map_err(|_| {
                    AppError::validation(
                        "publish_part_missing",
                        format!("missing publish part {n}"),
                    )
                })?;
            let json = json_from_part_bytes(&bytes, None)?;
            let payload = parse_part_payload(&json).map_err(|err| {
                AppError::validation("publish_part_invalid", format!("part {n}: {err}"))
            })?;
            parts.push(payload);
        }

        let incoming: HashSet<Uuid> = parts.iter().map(|part| part.document_id).collect();
        let cloud_ws = row.cloud_workspace_id;
        let owner_user = self.auth.user_id();
        let mut rebound = Vec::with_capacity(parts.len());
        for payload in &parts {
            if let Some(field) = payload
                .export
                .manifest
                .fingerprint
                .incompatible_field(&self.publish.fingerprint)
            {
                return Err(fingerprint_mismatch(field));
            }
            let mut batch = payload.export.batch.clone();
            batch.rebind_owner(owner_user, cloud_ws);
            self.publish
                .fingerprint
                .validate_batch(&batch)
                .map_err(|err| {
                    AppError::validation("publish_fingerprint_mismatch", err.to_string())
                })?;
            rebound.push((payload, batch));
        }

        // Write incoming docs before deleting stale ones so a mid-commit
        // failure leaves the previous replica intact (plus any finished
        // upserts) instead of a hole that live share links would hit.
        for (payload, batch) in rebound {
            docs.upsert_published_document(
                self.auth,
                PublishedDocumentUpsert {
                    document_id: payload.document_id,
                    workspace_id: cloud_ws,
                    filename: payload.filename.clone(),
                    mime_type: payload.mime_type.clone(),
                    summary: payload.summary.clone(),
                    chunk_count: payload.chunk_count,
                },
            )
            .await?;
            plane.replace_document_index(batch).await.map_err(|err| {
                AppError::internal(format!("replace_document_index: {err}"))
            })?;
        }

        let existing_docs = docs
            .list_documents(self.auth, Some(row.cloud_workspace_id), None)
            .await?;
        for existing in existing_docs {
            let Ok(id) = Uuid::parse_str(&existing.id) else {
                continue;
            };
            if incoming.contains(&id) {
                continue;
            }
            plane.delete_document_index(self.auth, id).await.map_err(|err| {
                AppError::internal(format!("stale index delete failed for {id}: {err}"))
            })?;
            docs.delete_document(self.auth, id).await?;
        }

        let ready = self
            .publish
            .store
            .mark_status(
                row.owner_user_id,
                row.local_workspace_id,
                PublishStatus::Ready,
                None,
                Some(Utc::now()),
            )
            .await?;
        self.delete_publish_parts(upload_id, row.expected_parts).await;
        Ok(status_response(&ready))
    }

    async fn fail_publish_session(&self, row: &WorkspacePublishRow, message: &str) {
        let _ = self
            .publish
            .store
            .mark_status(
                row.owner_user_id,
                row.local_workspace_id,
                PublishStatus::Failed,
                Some(message),
                None,
            )
            .await;
        if let Some(upload_id) = row.upload_id {
            self.delete_publish_parts(upload_id, row.expected_parts).await;
        }
    }

    async fn delete_publish_parts(&self, upload_id: Uuid, expected_parts: i32) {
        for n in 0..expected_parts.max(0) as u32 {
            let _ = self
                .object_store()
                .delete(&part_object_path(upload_id, n))
                .await;
        }
    }

    pub async fn get_publish_status(
        &self,
        local_workspace_id: Uuid,
    ) -> Result<PublishStatusResponse, AppError> {
        let row = self
            .publish
            .store
            .get_by_local(self.owner_uuid(), local_workspace_id)
            .await?;
        Ok(row
            .map(|row| status_response(&row))
            .unwrap_or(PublishStatusResponse {
                status: PublishStatus::Never,
                cloud_workspace_id: None,
                last_published_at: None,
                error: None,
                expected_parts: None,
            }))
    }

    pub async fn export_publish_list(
        &self,
        workspace_id: Uuid,
    ) -> Result<PublishExportListResponse, AppError> {
        let export = self.publish.retrieval_export.clone().ok_or_else(|| {
            AppError::upstream_unavailable("retrieval export is not available on this node")
        })?;
        let document_ids = export
            .list_indexed_document_ids(self.auth, Some(workspace_id))
            .await
            .map_err(|err| AppError::internal(format!("list indexed documents: {err}")))?;
        Ok(PublishExportListResponse {
            fingerprint: self.publish.fingerprint.clone(),
            document_ids,
        })
    }

    pub async fn export_publish_document(
        &self,
        workspace_id: Uuid,
        document_id: Uuid,
    ) -> Result<PublishPartPayload, AppError> {
        let export = self.publish.retrieval_export.clone().ok_or_else(|| {
            AppError::upstream_unavailable("retrieval export is not available on this node")
        })?;
        let exported = export
            .export_document_index(ExportDocumentRequest {
                auth: self.auth.clone(),
                document_id,
                workspace_id: Some(workspace_id),
                fingerprint: self.publish.fingerprint.clone(),
            })
            .await
            .map_err(|err| AppError::validation("publish_export_failed", err.to_string()))?;

        let mut filename = document_id.to_string();
        let mut mime_type = "application/octet-stream".to_string();
        let mut status = "completed".to_string();
        let mut chunk_count = exported.manifest.text_chunk_count;
        let mut summary = None;
        if let Some(docs) = self.storage.document_store() {
            if let Ok(rows) = docs
                .list_documents(self.auth, Some(workspace_id), Some(document_id))
                .await
            {
                if let Some(doc) = rows.into_iter().next() {
                    filename = doc.file_name;
                    mime_type = doc.mime_type;
                    status = doc.status.as_str().to_string();
                    chunk_count = doc.chunk_count;
                }
            }
            if let Ok(Some(content)) = docs.get_document_content(self.auth, document_id).await {
                summary = content.summary;
            }
        }
        Ok(PublishPartPayload {
            document_id,
            filename,
            mime_type,
            status,
            summary,
            chunk_count,
            export: exported,
        })
    }
}

fn status_response(row: &WorkspacePublishRow) -> PublishStatusResponse {
    PublishStatusResponse {
        status: row.status,
        cloud_workspace_id: Some(row.cloud_workspace_id),
        last_published_at: row.last_published_at,
        error: row.error.clone(),
        expected_parts: Some(row.expected_parts),
    }
}
