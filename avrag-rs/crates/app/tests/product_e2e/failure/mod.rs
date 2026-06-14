//! Failure-mode E2E tests — provider down, timeouts, embedding errors.

pub(crate) use crate::product_e2e::e2e_gate::require_integration_suite;

pub mod embedding_down;
pub mod provider_down;
pub mod search_degrade;
pub mod timeout;
