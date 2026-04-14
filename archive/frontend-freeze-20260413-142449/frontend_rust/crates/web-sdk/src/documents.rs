//! Documents API client

use crate::{ApiClient, dtos::*};

impl ApiClient {
    /// GET /api/v1/documents
    pub async fn list_documents(&self) -> anyhow::Result<DocumentsResponse> {
        self.get("/api/v1/documents").await
    }

    /// PUT /api/v1/documents/{document_id}
    pub async fn update_document(
        &self,
        document_id: &str,
        filename: Option<String>,
        notebook_id: Option<String>,
    ) -> anyhow::Result<Document> {
        #[derive(serde::Serialize)]
        struct Body {
            filename: Option<String>,
            notebook_id: Option<String>,
        }
        self.put(
            &format!("/api/v1/documents/{}", document_id),
            &Body {
                filename,
                notebook_id,
            },
        )
        .await
    }

    /// DELETE /api/v1/documents/{document_id}
    pub async fn delete_document(&self, document_id: &str) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!("/api/v1/documents/{}", document_id))
            .await
    }

    /// GET /api/v1/documents/{document_id}/status
    pub async fn get_document_status(
        &self,
        document_id: &str,
    ) -> anyhow::Result<DocumentStatusResponse> {
        self.get(&format!("/api/v1/documents/{}/status", document_id))
            .await
    }

    /// GET /api/v1/documents/{document_id}/content
    pub async fn get_document_content(
        &self,
        document_id: &str,
    ) -> anyhow::Result<DocumentContentResponse> {
        self.get(&format!("/api/v1/documents/{}/content", document_id))
            .await
    }

    /// GET /api/v1/documents/{document_id}/parsed-preview
    pub async fn get_parsed_preview(
        &self,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> anyhow::Result<ParsedPreviewResponse> {
        self.get(&format!(
            "/api/v1/documents/{}/parsed-preview?cursor={}&limit={}",
            document_id, cursor, limit
        ))
        .await
    }

    /// POST /api/v1/documents/{document_id}/reindex
    pub async fn reindex_document(&self, document_id: &str) -> anyhow::Result<EmptyResponse> {
        self.post(
            &format!("/api/v1/documents/{}/reindex", document_id),
            &EmptyResponse {},
        )
        .await
    }

    /// POST /api/v1/documents/{document_id}/complete-upload
    pub async fn complete_upload(&self, document_id: &str) -> anyhow::Result<EmptyResponse> {
        self.post(
            &format!("/api/v1/documents/{}/complete-upload", document_id),
            &EmptyResponse {},
        )
        .await
    }
}
