pub mod dense;
pub mod doc_metadata;
pub mod doc_summary;
pub mod graph;
pub mod index_lookup;
pub mod lexical;

use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::ScoredChunk;
use common::{ToolCall, ToolResult, ToolStatus};

use crate::RagRuntime;

/// Dispatch a single ToolCall to its pipeline.
pub async fn dispatch(
    runtime: &RagRuntime,
    auth: &AuthContext,
    call: &ToolCall,
) -> ToolResult {
    match call.tool.as_str() {
        "dense_retrieval" => dense::run(runtime, auth, &call.args).await,
        "lexical_retrieval" => lexical::run(runtime, auth, &call.args).await,
        "graph_retrieval" => graph::run(runtime, auth, &call.args).await,
        "index_lookup" => index_lookup::run(runtime, auth, &call.args).await,
        "doc_summary" => doc_summary::run(runtime, auth, &call.args).await,
        "doc_metadata" => doc_metadata::run(runtime, auth, &call.args).await,
        other => ToolResult {
            tool: other.to_string(),
            version: call.version.clone(),
            status: ToolStatus::NotImplemented,
            data: None,
            trace: None,
        },
    }
}

/// Dispatch multiple ToolCalls in parallel.
pub async fn dispatch_all(
    runtime: &RagRuntime,
    auth: &AuthContext,
    calls: Vec<ToolCall>,
) -> Vec<ToolResult> {
    let futures = calls
        .into_iter()
        .map(|call| {
            async move { dispatch(runtime, auth, &call).await }
        })
        .collect::<Vec<_>>();

    futures_util::future::join_all(futures).await
}

pub(crate) fn scored_chunk_to_json(chunk: &ScoredChunk) -> serde_json::Value {
    serde_json::json!({
        "chunk_id": chunk.chunk_id.to_string(),
        "doc_id": chunk.doc_id.to_string(),
        "text": chunk.content,
        "score": chunk.score,
        "page": chunk.page,
        "source": chunk.source,
    })
}

fn error_result(tool: &str, error: String) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Error,
        data: Some(serde_json::json!({ "error": error })),
        trace: None,
    }
}
