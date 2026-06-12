//! Product E2E tests — HTTP black-box, full pipeline, tiered assertions.
//!
//! Run smoke (PR level, mock LLM/Search/Embedding):
//!   E2E_MODE=smoke cargo test -p app --test product_e2e --features product-e2e smoke:: -- --test-threads=1
//!
//! Run integration (main branch, real infra):
//!   E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1

#[cfg(feature = "product-e2e")]
#[path = "product_e2e/mod.rs"]
mod product_e2e;

#[cfg(feature = "product-e2e")]
pub use product_e2e::*;

#[cfg(feature = "product-e2e")]
pub use product_e2e::llm_real;

#[cfg(not(feature = "product-e2e"))]
mod product_e2e_skipped {
    #[test]
    fn product_e2e_requires_product_e2e_feature() {}
}
