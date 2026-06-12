//! Integration E2E tests — main branch, real infra, P1 + P2 cases.

pub(crate) use crate::product_e2e::e2e_gate::require_integration_suite;

pub mod bad_file;
pub mod concurrent_query;
pub mod duplicate_upload;
pub mod embedding_cache;
pub mod format_output;
pub mod ingestion_full;
pub mod multi_doc;
pub mod streaming_chat;
