use crate::store::SubtexStore;
use async_trait::async_trait;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, MultimodalSearchRequest, RetrievalReadPort,
    ScoredChunk, TextDenseSearchRequest,
};

/// The Subtex store speaks the retrieval read contract, so the existing
/// RagRuntime tool dispatch can query it without changes. Search bodies are
/// filled in by the W1 indexing slice; M1 has no multimodal layer.
#[async_trait]
impl RetrievalReadPort for SubtexStore {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, _request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        Ok(Bm25SearchOutput {
            chunks: Vec::new(),
            trace: Bm25SearchTrace {
                backend: "subtex-sqlite".to_string(),
                raw_hit_count: 0,
                hydrated_hit_count: 0,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use subtex_core::RootHandle;
    use tempfile::TempDir;

    #[tokio::test]
    async fn read_port_roundtrips_with_empty_layers() {
        let dir = TempDir::new().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let handle = RootHandle::with_data_dir(&root, dir.path().join("data")).unwrap();
        let store = SubtexStore::open(&handle).unwrap();

        let output = store
            .search_bm25(Bm25SearchRequest {
                auth: test_auth(),
                query: "anything".to_string(),
                doc_ids: None,
                limit: 5,
            })
            .await
            .unwrap();
        assert_eq!(output.chunks.len(), 0);
        assert_eq!(output.trace.backend, "subtex-sqlite");

        let dense = store
            .search_text_dense(TextDenseSearchRequest {
                auth: test_auth(),
                query_vector: vec![0.0; 1024],
                doc_ids: None,
                limit: 5,
            })
            .await
            .unwrap();
        assert!(dense.is_empty());
    }

    fn test_auth() -> contracts::auth_runtime::AuthContext {
        use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
        AuthContext::new(UserId::from(uuid::Uuid::from_u128(1)), SubjectKind::System)
    }
}
