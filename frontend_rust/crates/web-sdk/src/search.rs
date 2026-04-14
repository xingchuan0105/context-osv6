//! Search API client

use serde::{Deserialize, Serialize};

use crate::{
    ApiClient,
    dtos::{ChatSession, Notebook, SourceRow},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResponse {
    #[serde(default)]
    pub notebooks: Vec<Notebook>,
    #[serde(default)]
    pub sessions: Vec<ChatSession>,
    #[serde(default)]
    pub sources: Vec<SourceRow>,
}

impl ApiClient {
    /// GET /api/v1/search?q=...
    pub async fn search(&self, query: &str) -> anyhow::Result<SearchResponse> {
        let path = format!("/api/v1/search?q={}", urlencoding::encode(query));
        self.get(&path).await
    }
}
