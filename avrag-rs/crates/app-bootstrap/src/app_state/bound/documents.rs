//! Bound face — documents.

use app_core::{AnalyticsServiceCtx, StorageContext};
use contracts::auth_runtime::AuthContext;
use futures::Stream;


pub struct BoundDocuments<'a> {
    pub(crate) docs: &'a app_documents::DocumentContext,
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
    pub(crate) billing: &'a app_billing::BillingContext,
    pub(crate) analytics: &'a AnalyticsServiceCtx,
}

impl<'a> BoundDocuments<'a> {
    pub async fn list_notebooks(&self) -> Vec<contracts::notebooks::Notebook> {
        self.docs
            .list_notebooks(self.auth, self.storage)
            .await
    }

    pub async fn get_notebook(
        &self,
        workspace_id: &str,
    ) -> Option<contracts::notebooks::Notebook> {
        self.docs
            .get_notebook(self.auth, self.storage, workspace_id)
            .await
    }

    pub async fn create_notebook(
        &self,
        req: common::CreateNotebookRequest,
    ) -> Result<contracts::notebooks::Notebook, common::AppError> {
        self.docs
            .create_notebook(self.auth, self.storage, self.analytics, req)
            .await
    }

    pub async fn update_notebook(
        &self,
        workspace_id: &str,
        req: common::UpdateNotebookRequest,
    ) -> Result<contracts::notebooks::Notebook, common::AppError> {
        self.docs
            .update_notebook(self.auth, self.storage, workspace_id, req)
            .await
    }

    pub async fn delete_notebook(
        &self,
        workspace_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .delete_notebook(self.auth, self.storage, workspace_id)
            .await
    }

    pub async fn list_documents(
        &self,
        workspace_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Vec<common::Document> {
        self.docs
            .list_documents(self.auth, self.storage, workspace_id, document_id)
            .await
    }

    pub async fn create_document_upload(
        &self,
        workspace_id: &str,
        req: common::CreateDocumentRequest,
    ) -> Result<contracts::documents::CreateDocumentUploadResponse, common::AppError> {
        self.docs
            .create_document_upload(
                self.auth,
                self.storage,
                self.billing,
                self.analytics,
                workspace_id,
                req,
            )
            .await
    }

    pub async fn put_uploaded_document(
        &self,
        document_id: &str,
        body: Vec<u8>,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .put_uploaded_document(self.auth, self.storage, document_id, body)
            .await
    }

    pub async fn put_uploaded_document_stream<S, E>(
        &self,
        document_id: &str,
        stream: S,
    ) -> Result<common::StatusOnlyResponse, common::AppError>
    where
        S: Stream<Item = std::result::Result<bytes::Bytes, E>>
            + Send
            + Sync
            + Unpin
            + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.docs
            .put_uploaded_document_stream(self.auth, self.storage, document_id, stream)
            .await
    }

    pub async fn complete_document_upload(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .complete_document_upload(self.auth, self.storage, self.analytics, document_id)
            .await
    }

    pub async fn transition_document_status(
        &self,
        document_id: &str,
        status: contracts::documents::DocumentStatus,
    ) -> Result<(), common::AppError> {
        self.docs
            .transition_document_status(self.auth, self.storage, document_id, status)
            .await
    }

    pub async fn simulate_ingestion(&self, document_id: String) {
        self.docs
            .simulate_ingestion(self.auth, self.storage, document_id)
            .await
    }

    pub async fn update_document(
        &self,
        document_id: &str,
        req: common::UpdateDocumentRequest,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .update_document(self.auth, self.storage, document_id, req)
            .await
    }

    pub async fn delete_document(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .delete_document(self.auth, self.storage, document_id)
            .await
    }

    pub async fn reindex_document(
        &self,
        document_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.docs
            .reindex_document(self.auth, self.storage, self.analytics, document_id)
            .await
    }

    pub async fn get_document_content(
        &self,
        document_id: &str,
    ) -> Result<common::DocumentContentResponse, common::AppError> {
        self.docs
            .get_document_content(self.auth, self.storage, document_id)
            .await
    }

    pub async fn get_parsed_preview(
        &self,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<common::ParsedPreviewResponse, common::AppError> {
        self.docs
            .get_parsed_preview(self.auth, self.storage, document_id, cursor, limit)
            .await
    }

    pub async fn add_url_source(
        &self,
        workspace_id: &str,
        req: common::AddUrlSourceRequest,
    ) -> Result<contracts::documents::CreateDocumentUploadResponse, common::AppError> {
        self.docs
            .add_url_source(
                self.auth,
                self.storage,
                self.billing,
                self.analytics,
                workspace_id,
                req,
            )
            .await
    }

    pub async fn list_sources(&self, workspace_id: Option<&str>) -> Vec<common::SourceRow> {
        self.docs
            .list_sources(self.auth, self.storage, workspace_id)
            .await
    }
}

