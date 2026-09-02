use crate::context::ChatContext;
use app_core::parse_uuid_or_app_error;
use common::{AppError, StatusOnlyResponse};
use contracts::documents::{
    CreateDocumentRequest, CreateDocumentUploadResponse, SessionFilesResponse,
};
use uuid::Uuid;

/// Chat-first W2b: files bound to a Conversation. The upload admission is
/// shared with workspace uploads; the scope is the conversation binding only.
impl ChatContext {
    fn require_document_store(
        &self,
    ) -> Result<std::sync::Arc<dyn app_core::DocumentStorePort>, AppError> {
        self.storage
            .document_store()
            .ok_or_else(|| AppError::internal("document store is not configured"))
    }

    async fn require_owned_session(&self, session_id: &str) -> Result<Uuid, AppError> {
        let session_id =
            parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
        let session = self
            .get_session(&session_id.to_string())
            .await
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        let _ = session;
        Ok(session_id)
    }

    pub async fn create_session_file_upload(
        &self,
        session_id: &str,
        req: CreateDocumentRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        let conversation_id = self.require_owned_session(session_id).await?;
        app_documents::ensure_document_upload_allowed(
            &self.auth,
            &self.storage,
            &self.billing,
            &req.filename,
            &req.mime_type,
            req.file_size,
        )
        .await?;

        let store = self.require_document_store()?;
        let document = store
            .create_session_document(
                &self.auth,
                conversation_id,
                req.filename.trim(),
                req.file_size,
                &req.mime_type,
            )
            .await?;
        let document_uuid = parse_uuid_or_app_error(
            &document.id,
            "document_not_found",
            "document not found",
        )?;
        let seed = store
            .get_document_task_seed(&self.auth, document_uuid)
            .await?
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        Ok(CreateDocumentUploadResponse {
            document_id: document.id.clone(),
            upload_url: self
                .storage
                .signed_upload_url(&document.id, &seed.object_path, None)?,
            status: "pending".to_string(),
        })
    }

    pub async fn list_session_files(&self, session_id: &str) -> Result<SessionFilesResponse, AppError> {
        let conversation_id = self.require_owned_session(session_id).await?;
        let store = self.require_document_store()?;
        let files = store
            .list_session_files(&self.auth, conversation_id)
            .await?;
        Ok(SessionFilesResponse { files })
    }

    pub async fn delete_session_file(
        &self,
        session_id: &str,
        binding_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let conversation_id = self.require_owned_session(session_id).await?;
        let binding_id =
            parse_uuid_or_app_error(binding_id, "session_file_not_found", "session file not found")?;
        let store = self.require_document_store()?;
        let deleted = store
            .delete_session_file_binding(&self.auth, conversation_id, binding_id)
            .await?;
        let Some(artifact_id) = deleted else {
            return Err(AppError::not_found(
                "session_file_not_found",
                "session file not found",
            ));
        };
        // W2e: removing the last binding hands the artifact to the async
        // full-cleanup flow; any remaining binding keeps it untouched.
        let artifact_uuid = parse_uuid_or_app_error(
            &artifact_id,
            "session_file_not_found",
            "session file not found",
        )?;
        let _ = store
            .delete_document_if_unbound(&self.auth, artifact_uuid)
            .await?;
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }
}
