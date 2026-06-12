pub use app_documents::document_is_deleting_or_deleted;

use super::AppState;

impl AppState {
    pub async fn list_documents(
        &self,
        notebook_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Vec<common::Document> {
        self.documents
            .list_documents(&self.auth, &self.storage, notebook_id, document_id)
            .await
    }

    pub async fn create_document_upload(
        &self,
        notebook_id: &str,
        req: common::CreateDocumentRequest,
    ) -> Result<common::CreateDocumentUploadResponse, common::AppError> {
        self.documents
            .create_document_upload(
                &self.auth,
                &self.storage,
                &self.billing,
                &self.analytics,
                notebook_id,
                req,
            )
            .await
    }

    pub async fn put_uploaded_document(
        &self,
        document_id: &str,
        body: Vec<u8>,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.documents
            .put_uploaded_document(&self.auth, &self.storage, document_id, body)
            .await
    }

    pub async fn put_uploaded_document_stream<S, E>(
        &self,
        document_id: &str,
        stream: S,
    ) -> Result<common::StatusOnlyResponse, common::AppError>
    where
        S: futures::Stream<Item = std::result::Result<bytes::Bytes, E>>
            + Send
            + Sync
            + Unpin
            + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.documents
            .put_uploaded_document_stream(&self.auth, &self.storage, document_id, stream)
            .await
    }

    pub async fn complete_document_upload(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.documents
            .complete_document_upload(&self.auth, &self.storage, &self.analytics, document_id)
            .await
    }

    pub async fn transition_document_status(
        &self,
        document_id: &str,
        status: common::DocumentStatus,
    ) -> Result<(), common::AppError> {
        self.documents
            .transition_document_status(&self.auth, &self.storage, document_id, status)
            .await
    }

    pub async fn simulate_ingestion(&self, document_id: String) {
        self.documents
            .simulate_ingestion(&self.auth, &self.storage, document_id)
            .await
    }

    pub async fn update_document(
        &self,
        document_id: &str,
        req: common::UpdateDocumentRequest,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.documents
            .update_document(&self.auth, &self.storage, document_id, req)
            .await
    }

    pub async fn delete_document(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.documents
            .delete_document(&self.auth, &self.storage, document_id)
            .await
    }

    pub async fn reindex_document(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.documents
            .reindex_document(&self.auth, &self.storage, &self.analytics, document_id)
            .await
    }

    pub async fn get_document_content(
        &self,
        document_id: &str,
    ) -> Result<common::DocumentContentResponse, common::AppError> {
        self.documents
            .get_document_content(&self.auth, &self.storage, document_id)
            .await
    }

    pub async fn get_parsed_preview(
        &self,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<common::ParsedPreviewResponse, common::AppError> {
        self.documents
            .get_parsed_preview(&self.auth, &self.storage, document_id, cursor, limit)
            .await
    }
}
