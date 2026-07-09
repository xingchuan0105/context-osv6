use async_trait::async_trait;
use contracts::auth_runtime::{AuthContext, OrgId, SubjectKind};
use avrag_rag_core::RagRuntime;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, GraphSearchOutput, GraphSearchRequest,
    MultimodalSearchRequest, RetrievalReadPort, ScoredChunk, TextDenseSearchRequest,
};
use contracts::{GraphHint, GraphRetrievalArgs, ToolResult, ToolStatus};
use std::sync::Arc;
use uuid::Uuid;

/// Mock data plane that simulates Milvus cross-tenant rejection.
///
/// The "data owner" org is fixed at construction time.  Any graph search
/// whose `tenant_org_id` does not match the data owner is rejected with
/// the same error message that `TenantContext::verify_access` produces.
struct CrossTenantBlockingDataPlane {
    data_owner_org_id: String,
}

#[async_trait]
impl RetrievalReadPort for CrossTenantBlockingDataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, _request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        Ok(Bm25SearchOutput {
            chunks: Vec::new(),
            trace: avrag_retrieval_data_plane::Bm25SearchTrace {
                backend: "stub".to_string(),
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

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        if request.tenant_org_id != self.data_owner_org_id {
            return Err(anyhow::anyhow!("cross_tenant_access_denied"));
        }
        Ok(GraphSearchOutput::default())
    }
}

fn test_runtime(data_owner_org_id: &str) -> RagRuntime {
    let config = avrag_rag_core::test_doubles::test_rag_config();
    RagRuntime::with_data_plane(
        config,
        Arc::new(CrossTenantBlockingDataPlane {
            data_owner_org_id: data_owner_org_id.to_string(),
        }),
    )
}

fn make_auth(org_id: Uuid) -> AuthContext {
    AuthContext::new(OrgId::new(org_id), SubjectKind::System)
}

#[tokio::test]
async fn graph_retrieval_rejects_cross_tenant_access() {
    let data_owner = Uuid::from_u128(0x0001);
    let attacker = Uuid::from_u128(0x0002);

    let runtime = test_runtime(&data_owner.to_string());
    let auth = make_auth(attacker);

    let args = serde_json::to_value(GraphRetrievalArgs {
        graph_hints: vec![GraphHint {
            subject: Some("Alice".to_string()),
            predicate: Some("knows".to_string()),
            object: Some("Bob".to_string()),
        }],
        placeholder_triplets: Vec::new(),
        relation_limit: 5,
        supporting_chunk_limit: 5,
        hop_limit: 2,
        fan_out_limit: 10,
        query: None,
        doc_scope: Vec::new(),
    })
    .unwrap();

    let result: ToolResult =
        avrag_rag_core::runtime::tools::graph::run(&runtime, &auth, &args).await;

    assert_eq!(result.tool, "graph_retrieval");
    assert_eq!(
        result.status,
        ToolStatus::Error,
        "graph_retrieval must return Error for cross-tenant requests"
    );
    let error_msg = result
        .data
        .as_ref()
        .and_then(|d| d.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        error_msg.contains("cross_tenant_access_denied"),
        "error message should indicate cross-tenant denial, got: {}",
        error_msg
    );
}

#[tokio::test]
async fn graph_retrieval_allows_same_tenant_access() {
    let data_owner = Uuid::from_u128(0x0001);

    let runtime = test_runtime(&data_owner.to_string());
    let auth = make_auth(data_owner);

    let args = serde_json::to_value(GraphRetrievalArgs {
        graph_hints: vec![GraphHint {
            subject: Some("Alice".to_string()),
            predicate: Some("knows".to_string()),
            object: Some("Bob".to_string()),
        }],
        placeholder_triplets: Vec::new(),
        relation_limit: 5,
        supporting_chunk_limit: 5,
        hop_limit: 2,
        fan_out_limit: 10,
        query: None,
        doc_scope: Vec::new(),
    })
    .unwrap();

    let result: ToolResult =
        avrag_rag_core::runtime::tools::graph::run(&runtime, &auth, &args).await;

    assert_eq!(result.tool, "graph_retrieval");
    assert_eq!(
        result.status,
        ToolStatus::Ok,
        "graph_retrieval must return Ok for same-tenant requests"
    );
}
