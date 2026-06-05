//! Smoke E2E tests — PR level, 3 P0 cases.
//!
//! - ingestion_smoke.rs: upload → wait → verify PG data
//! - rag_smoke.rs: upload → RAG query → verify doc citation
//! - search_smoke.rs: open query → verify web citation

pub mod auth_boundary;
pub mod ingestion_smoke;
pub mod rag_smoke;
pub mod search_smoke;

use crate::product_e2e::TestContext;

/// Blocking backend launcher for frontend E2E tests.
///
/// When run via `cargo test -p app --test product_e2e backend_launcher -- --ignored`,
/// this test starts the full backend stack (PG + Milvus + worker + HTTP server),
/// writes the base URL to `/tmp/e2e-backend.url`, and blocks until the process
/// is killed. Playwright's globalSetup consumes this URL.
#[tokio::test]
#[ignore = "blocking backend launcher for frontend e2e"]
async fn backend_launcher() {
    let ctx = TestContext::new_with_real_llm().await;
    let url = ctx.base_url.clone();
    std::fs::write("/tmp/e2e-backend.url", &url).expect("write backend url");
    eprintln!("[backend_launcher] backend ready at {url}");
    eprintln!("[backend_launcher] blocking until process is killed...");

    // Block forever. When the parent Playwright process kills us,
    // `ctx` drops and cleans up containers.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
