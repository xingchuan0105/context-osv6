use common::AppError;
use contracts::documents::CitationLookupResponse;

use super::AppState;

impl AppState {
    pub async fn lookup_citation(
        &self,
        session_id: &str,
        message_id: i64,
        citation_id: i64,
    ) -> Result<CitationLookupResponse, AppError> {
        self.chat_ctx()
            .lookup_citation(session_id, message_id, citation_id)
            .await
    }

    pub async fn get_citation_asset(&self, asset_id: &str) -> Result<(Vec<u8>, String), AppError> {
        self.chat_ctx().get_citation_asset(asset_id).await
    }
}
