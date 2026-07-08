use std::sync::Arc;

use app_core::PostgresHealthPort;
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;

pub struct PgHealthAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgHealthAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl PostgresHealthPort for PgHealthAdapter {
    async fn ping(&self) -> Result<(), String> {
        self.repo
            .bootstrap()
            .ping()
            .await
            .map_err(|error| error.to_string())
    }
}
