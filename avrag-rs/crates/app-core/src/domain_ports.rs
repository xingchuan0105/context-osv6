use async_trait::async_trait;
use avrag_auth::AuthContext;
use common::AppError;

use crate::StorageContext;

/// Retrieval boundary for chat/RAG — implementations live in rag-core + storage adapters.
pub use avrag_retrieval_data_plane::RetrievalDataPlane as RetrievalPort;

pub use crate::admin_store::AdminStorePort;
pub use crate::auth_store::AuthStorePort;
pub use crate::billing_quota::BillingQuotaPort;
pub use crate::chat_persistence::ChatPersistencePort;
pub use crate::document_store::DocumentStorePort;

/// Validates that document IDs belong to the caller's notebook scope.
#[async_trait]
pub trait DocumentScopeValidator: Send + Sync {
    async fn validate_document_scope(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        document_ids: &[String],
    ) -> Result<(), AppError>;
}

/// Resolves citation markers to source metadata for chat responses.
#[async_trait]
pub trait CitationResolver: Send + Sync {
    async fn lookup_citation(
        &self,
        session_id: &str,
        citation_id: &str,
    ) -> Result<Option<common::Citation>, AppError>;
}
