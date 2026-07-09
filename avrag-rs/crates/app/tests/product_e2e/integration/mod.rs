//! Integration E2E tests — main branch, real infra, P1 + P2 cases.

pub(crate) use crate::product_e2e::e2e_gate::require_integration_suite;

pub mod bad_file;
pub mod concurrent_query;
pub mod document_lifecycle;
pub mod duplicate_upload;
pub mod embedding_cache;
pub mod format_output;
pub mod ingestion_full;
pub mod liteparse_pdf_e2e;
pub mod mcp_agent_flow;
pub mod mcp_auth_boundary;
pub mod multi_doc;
pub mod office_docx_e2e;
pub mod office_docx_staging_e2e;
pub mod office_pptx_e2e;
pub mod office_pptx_staging_e2e;
pub mod office_xlsx_e2e;
pub mod office_xlsx_staging_e2e;
pub mod paddle_image_e2e;
pub mod quota_boundary;
pub mod rate_limit_boundary;
pub mod streaming_chat;
