//! Internal Rust runtime types for avrag services.
//!
//! Wire DTOs live in `contracts/` — import them directly from that crate.
//! This crate holds service-only helpers (errors, content store, etc.).
//! New cross-language fields belong in `contracts/` first; see `CONTEXT.md`.

pub mod chat;
pub mod content_store;
pub mod docscope;
pub mod documents;
pub mod errors;
pub mod guards_access;
pub mod identity;
pub mod key_vault;
pub mod workspace_requests;
pub mod ssrf;
pub mod text_segment;
pub mod util;

pub use chat::{
    answer_blocks_from_rendered_answer, answer_blocks_to_markup, plain_text_answer_blocks,
};
pub use content_store::{ContentStore, ContentStoreError, IndexedChunk};
pub use docscope::{
    DocScopeMetadata, DocScopeProfile, Domain, Era, Genre, SummaryMetadata, SummaryOutput,
};
pub use documents::{
    AddUrlSourceRequest, CreateDocumentRequest, Document, DocumentContentResponse,
    DocumentMetadata, DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse, SourceRow,
    SourcesResponse, StatusOnlyResponse, TocEntry, UpdateDocumentRequest,
};
pub use errors::{ApiError, ApiResponse, AppError, ErrorBody};
pub use guards_access::{
    AnswerContextChunk, ApiKeyListResponse, ApiKeyRow, CreateApiKeyRequest, CreateApiKeyResponse,
    InputGuardType, NotificationRow, NotificationsResponse, OutputGuardType, ShareTokenResponse,
};
pub use identity::{OrgId, UserId, default_org_id, default_rag_agent, default_user_id};
pub use workspace_requests::{CreateWorkspaceRequest, UpdateWorkspaceRequest};
pub use ssrf::{SsrfError, validate_http_url, validate_http_url_with_dns};
pub use text_segment::{merge_search_tokens, rrf_merge, segment_for_fts};
pub use util::{
    estimate_token_count, infer_image_extension, infer_mime_type, is_remote_url, new_id,
    now_rfc3339,
};
