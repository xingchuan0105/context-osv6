use avrag_auth::OrgId;
use common::AppError;
use uuid::Uuid;

use super::AppState;

#[derive(Debug, Clone)]
pub struct WorkspaceApiKeyAuth {
    pub key_id: Uuid,
    pub org_id: OrgId,
    pub notebook_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub rate_limit_rpm: u32,
}

impl AppState {
    pub async fn validate_workspace_api_key(
        &self,
        plaintext_key: &str,
    ) -> Result<Option<WorkspaceApiKeyAuth>, AppError> {
        if let Some(repo) = self.postgres_repo() {
            let validated = repo
                .validate_api_key(plaintext_key)
                .await
                .map_err(crate::pg_error::map_pg_error)?;
            return Ok(validated.map(|key| WorkspaceApiKeyAuth {
                key_id: key.id,
                org_id: key.org_id,
                notebook_id: key.notebook_id,
                permissions: key.permissions,
                rate_limit_rpm: key.rate_limit_rpm,
            }));
        }

        Ok(self
            .admin
            .validate_api_key(&self.storage, plaintext_key)
            .await?
            .map(|record| WorkspaceApiKeyAuth {
                key_id: record.id,
                org_id: record.org_id,
                notebook_id: record.notebook_id,
                permissions: record.permissions,
                rate_limit_rpm: record.rate_limit_rpm,
            }))
    }
}
