//! Notebooks API client

use crate::{ApiClient, dtos::*};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawMemberRow {
    id: String,
    user_id: Option<String>,
    email: Option<String>,
    access_level: serde_json::Value,
    invite_status: String,
    invited_at: i64,
}

impl ApiClient {
    /// GET /api/v1/notebooks
    pub async fn list_notebooks(&self) -> anyhow::Result<NotebookListResponse> {
        self.get("/api/v1/notebooks").await
    }

    /// POST /api/v1/notebooks
    pub async fn create_notebook(
        &self,
        req: &CreateNotebookRequest,
    ) -> anyhow::Result<NotebookResponse> {
        self.post("/api/v1/notebooks", req).await
    }

    /// GET /api/v1/notebooks/{notebook_id}
    pub async fn get_notebook(&self, notebook_id: &str) -> anyhow::Result<NotebookResponse> {
        self.get(&format!("/api/v1/notebooks/{}", notebook_id))
            .await
    }

    /// PUT /api/v1/notebooks/{notebook_id}
    pub async fn update_notebook(
        &self,
        notebook_id: &str,
        req: &UpdateNotebookRequest,
    ) -> anyhow::Result<NotebookResponse> {
        self.put(&format!("/api/v1/notebooks/{}", notebook_id), req)
            .await
    }

    /// DELETE /api/v1/notebooks/{notebook_id}
    pub async fn delete_notebook(&self, notebook_id: &str) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!("/api/v1/notebooks/{}", notebook_id))
            .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/api-keys
    pub async fn list_api_keys(&self, notebook_id: &str) -> anyhow::Result<ApiKeyListResponse> {
        self.get(&format!("/api/v1/notebooks/{}/api-keys", notebook_id))
            .await
    }

    /// POST /api/v1/notebooks/{notebook_id}/api-keys
    pub async fn create_api_key(
        &self,
        notebook_id: &str,
        req: &CreateApiKeyRequest,
    ) -> anyhow::Result<CreateApiKeyResponse> {
        self.post(&format!("/api/v1/notebooks/{}/api-keys", notebook_id), req)
            .await
    }

    /// DELETE /api/v1/notebooks/{notebook_id}/api-keys/{key_id}
    pub async fn delete_api_key(
        &self,
        notebook_id: &str,
        key_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!(
            "/api/v1/notebooks/{}/api-keys/{}",
            notebook_id, key_id
        ))
        .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/members
    pub async fn list_members(&self, notebook_id: &str) -> anyhow::Result<MembersResponse> {
        #[derive(Deserialize)]
        struct MembersEnvelope {
            members: Vec<RawMemberRow>,
        }

        let resp: MembersEnvelope = self
            .get(&format!("/api/v1/notebooks/{}/members", notebook_id))
            .await?;
        Ok(MembersResponse {
            members: resp
                .members
                .into_iter()
                .map(|member| MemberRow {
                    member_id: member.id,
                    user_id: member.user_id.unwrap_or_default(),
                    email: member.email.unwrap_or_default(),
                    role: member
                        .access_level
                        .as_str()
                        .unwrap_or("viewer")
                        .to_string()
                        .to_lowercase(),
                    status: member.invite_status,
                    invited_at: member.invited_at.to_string(),
                })
                .collect(),
        })
    }

    /// POST /api/v1/notebooks/{notebook_id}/members/invite
    pub async fn invite_member(
        &self,
        notebook_id: &str,
        email: &str,
        role: &str,
    ) -> anyhow::Result<EmptyResponse> {
        #[derive(serde::Serialize)]
        struct Body {
            email: String,
            role: String,
        }
        let _: serde_json::Value = self
            .post(
                &format!("/api/v1/notebooks/{}/members/invite", notebook_id),
                &Body {
                    email: email.to_string(),
                    role: role.to_string(),
                },
            )
            .await?;
        Ok(EmptyResponse {})
    }

    /// POST /api/v1/notebooks/{notebook_id}/members/{member_id}/accept
    pub async fn accept_invite(
        &self,
        notebook_id: &str,
        member_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        let _: serde_json::Value = self
            .post(
                &format!(
                    "/api/v1/notebooks/{}/members/{}/accept",
                    notebook_id, member_id
                ),
                &EmptyResponse {},
            )
            .await?;
        Ok(EmptyResponse {})
    }

    /// POST /api/v1/notebooks/{notebook_id}/members/{member_id}/decline
    pub async fn decline_invite(
        &self,
        notebook_id: &str,
        member_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        let _: serde_json::Value = self
            .post(
                &format!(
                    "/api/v1/notebooks/{}/members/{}/decline",
                    notebook_id, member_id
                ),
                &EmptyResponse {},
            )
            .await?;
        Ok(EmptyResponse {})
    }

    /// DELETE /api/v1/notebooks/{notebook_id}/members/{member_id}
    pub async fn remove_member(
        &self,
        notebook_id: &str,
        member_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        let _: serde_json::Value = self
            .delete(&format!(
                "/api/v1/notebooks/{}/members/{}",
                notebook_id, member_id
            ))
            .await?;
        Ok(EmptyResponse {})
    }

    /// GET /api/v1/sources?notebook_id={notebook_id}
    pub async fn list_sources(&self, notebook_id: &str) -> anyhow::Result<SourcesResponse> {
        self.get(&format!("/api/v1/sources?notebook_id={}", notebook_id))
            .await
    }

    /// POST /api/v1/notebooks/{notebook_id}/documents
    pub async fn create_document_upload(
        &self,
        notebook_id: &str,
        req: &CreateDocumentRequest,
    ) -> anyhow::Result<CreateDocumentUploadResponse> {
        self.post(&format!("/api/v1/notebooks/{}/documents", notebook_id), req)
            .await
    }

    /// POST /api/v1/notebooks/{notebook_id}/sources/url
    pub async fn add_url_source(
        &self,
        notebook_id: &str,
        url: &str,
    ) -> anyhow::Result<CreateDocumentUploadResponse> {
        #[derive(serde::Serialize)]
        struct Body {
            url: String,
        }
        self.post(
            &format!("/api/v1/notebooks/{}/sources/url", notebook_id),
            &Body {
                url: url.to_string(),
            },
        )
        .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/analysis
    pub async fn get_notebook_analysis(
        &self,
        notebook_id: &str,
    ) -> anyhow::Result<NotebookAnalysisResponse> {
        self.get(&format!("/api/v1/notebooks/{}/analysis", notebook_id))
            .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/notes
    pub async fn list_notebook_notes(
        &self,
        notebook_id: &str,
    ) -> anyhow::Result<NotebookNoteListResponse> {
        self.get(&format!("/api/v1/notebooks/{}/notes", notebook_id))
            .await
    }

    /// GET /api/v1/notebooks/{notebook_id}/notes/{note_id}
    pub async fn get_notebook_note(
        &self,
        notebook_id: &str,
        note_id: &str,
    ) -> anyhow::Result<NotebookNoteResponse> {
        self.get(&format!(
            "/api/v1/notebooks/{}/notes/{}",
            notebook_id, note_id
        ))
        .await
    }

    /// POST /api/v1/notebooks/{notebook_id}/notes
    pub async fn create_notebook_note(
        &self,
        notebook_id: &str,
        req: &CreateNotebookNoteRequest,
    ) -> anyhow::Result<NotebookNoteResponse> {
        self.post(&format!("/api/v1/notebooks/{}/notes", notebook_id), req)
            .await
    }

    /// PUT /api/v1/notebooks/{notebook_id}/notes/{note_id}
    pub async fn update_notebook_note(
        &self,
        notebook_id: &str,
        note_id: &str,
        req: &UpdateNotebookNoteRequest,
    ) -> anyhow::Result<NotebookNoteResponse> {
        self.put(
            &format!("/api/v1/notebooks/{}/notes/{}", notebook_id, note_id),
            req,
        )
        .await
    }

    /// DELETE /api/v1/notebooks/{notebook_id}/notes/{note_id}
    pub async fn delete_notebook_note(
        &self,
        notebook_id: &str,
        note_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!(
            "/api/v1/notebooks/{}/notes/{}",
            notebook_id, note_id
        ))
        .await
    }

    /// POST /api/v1/notebooks/{notebook_id}/notes/{note_id}/promote-to-source
    pub async fn promote_notebook_note(
        &self,
        notebook_id: &str,
        note_id: &str,
    ) -> anyhow::Result<PromoteNotebookNoteResponse> {
        self.post(
            &format!(
                "/api/v1/notebooks/{}/notes/{}/promote-to-source",
                notebook_id, note_id
            ),
            &EmptyResponse {},
        )
        .await
    }
}
