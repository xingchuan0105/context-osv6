//! Smoke E2E tests — PR level, 3 P0 cases.
//!
//! - ingestion_smoke.rs: upload → wait → verify PG data
//! - rag_smoke.rs: upload → RAG query → verify doc citation
//! - search_smoke.rs: open query → verify web citation

pub mod ingestion_smoke;
pub mod rag_smoke;
pub mod search_smoke;
