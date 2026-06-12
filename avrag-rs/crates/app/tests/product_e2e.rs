//! Product E2E tests — HTTP black-box, full pipeline, tiered assertions.
//!
//! Run smoke (PR level, mock LLM/Search/Embedding):
//!   E2E_MODE=smoke cargo test -p app --test product_e2e smoke:: -- --test-threads=1
//!
//! Run integration (main branch, real infra):
//!   E2E_MODE=integration cargo test -p app --test product_e2e -- --test-threads=1

#[path = "product_e2e/mod.rs"]
pub mod product_e2e;

pub use product_e2e::*;

// Re-export the real-LLM sub-modules so their #[test] functions are
// discovered by the test runner under the `llm_real` namespace.
pub use product_e2e::llm_real;
