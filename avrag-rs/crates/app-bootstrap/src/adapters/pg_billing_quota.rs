use std::sync::Arc;

use app_billing::BillingContext;
use app_core::{BillingQuotaPort, DocumentStorePort};
use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
use common::AppError;
use uuid::Uuid;

pub struct PgBillingQuotaAdapter {
    billing: BillingContext,
    document_store: Arc<dyn DocumentStorePort>,
}

impl PgBillingQuotaAdapter {
    pub fn new(billing: BillingContext, document_store: Arc<dyn DocumentStorePort>) -> Self {
        Self {
            billing,
            document_store,
        }
    }
}

#[async_trait]
impl BillingQuotaPort for PgBillingQuotaAdapter {
    async fn ensure_storage_bytes_quota(
        &self,
        auth: &AuthContext,
        bytes: i64,
    ) -> Result<(), AppError> {
        self.billing
            .ensure_metric_quota(auth, "storage_bytes", bytes)
            .await
    }

    async fn notebook_exists(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, AppError> {
        Ok(self
            .document_store
            .get_workspace(auth, workspace_id)
            .await?
            .is_some())
    }
}
