#[derive(Debug, Clone)]
pub struct PgvectorConfig {
    pub text_vector_dim: usize,
    pub multimodal_vector_dim: usize,
    /// Session `hnsw.ef_search` (optional).
    pub hnsw_ef_search: Option<u32>,
}

impl Default for PgvectorConfig {
    fn default() -> Self {
        Self {
            text_vector_dim: 1024,
            multimodal_vector_dim: 1024,
            hnsw_ef_search: Some(40),
        }
    }
}
