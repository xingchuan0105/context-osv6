//! Testcontainers orchestration for Product E2E.
//!
//! Smoke and Integration both use real infrastructure:
//! - PostgreSQL (Testcontainers)
//! - Milvus Standalone (Testcontainers)
//! - Local filesystem object store (TempDir via ObjectStore trait)
//!
//! TODO(Phase 2): implement container lifecycle.

use std::path::Path;

/// Start PostgreSQL container and return connection URL.
pub async fn start_postgres() -> anyhow::Result<String> {
    // TODO(Phase 2): use testcontainers::postgres
    Ok("postgres://test:test@localhost:5432/test".to_string())
}

/// Start Milvus standalone container and return gRPC/HTTP URLs.
pub async fn start_milvus() -> anyhow::Result<MilvusEndpoints> {
    // TODO(Phase 2): use testcontainers::generic_image for milvus
    Ok(MilvusEndpoints {
        grpc: "localhost:19530".to_string(),
        http: "http://localhost:19530".to_string(),
    })
}

/// Milvus connection endpoints.
pub struct MilvusEndpoints {
    pub grpc: String,
    pub http: String,
}

/// Create a temporary object store directory.
pub fn create_temp_object_store() -> tempfile::TempDir {
    tempfile::tempdir().expect("create tempdir")
}

/// Load fixture content from `tests/product_e2e/fixtures/`.
pub fn load_fixture(name: &str) -> anyhow::Result<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/product_e2e/fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("failed to read fixture {}: {}", path.display(), e))
}
