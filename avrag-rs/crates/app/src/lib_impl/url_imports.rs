use common::{AddUrlSourceRequest, AppError, CreateDocumentUploadResponse, SourceRow};

use crate::lib_impl::AppState;

impl AppState {
    pub async fn add_url_source(
        &self,
        notebook_id: &str,
        req: AddUrlSourceRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        self.documents
            .add_url_source(
                &self.auth,
                &self.storage,
                &self.billing,
                &self.analytics,
                notebook_id,
                req,
            )
            .await
    }

    pub async fn list_sources(&self, notebook_id: Option<&str>) -> Vec<SourceRow> {
        self.documents
            .list_sources(&self.auth, &self.storage, notebook_id)
            .await
    }
}
