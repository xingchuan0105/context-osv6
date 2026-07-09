pub mod dense;
pub mod doc_metadata;
pub mod doc_profile;
pub mod doc_scan;
pub mod doc_summary;
pub mod graph;
pub mod index_lookup;
pub mod lexical;

use contracts::auth_runtime::AuthContext;
use avrag_retrieval_data_plane::ScoredChunk;
use contracts::{ToolCall, ToolResult, ToolStatus};

use crate::RagRuntime;

/// Dispatch a single ToolCall to its pipeline.
pub async fn dispatch(runtime: &RagRuntime, auth: &AuthContext, call: &ToolCall) -> ToolResult {
    match call.tool.as_str() {
        "dense_retrieval" => dense::run(runtime, auth, &call.args).await,
        "lexical_retrieval" => lexical::run(runtime, auth, &call.args).await,
        "graph_retrieval" => graph::run(runtime, auth, &call.args).await,
        "index_lookup" => index_lookup::run(runtime, auth, &call.args).await,
        "doc_summary" => doc_summary::run(runtime, auth, &call.args).await,
        "doc_metadata" => doc_metadata::run(runtime, auth, &call.args).await,
        "doc_profile" => doc_profile::run(runtime, auth, &call.args).await,
        "doc_scan" => doc_scan::run(runtime, auth, &call.args).await,
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
        .map(|call| async move { dispatch(runtime, auth, &call).await })
        .collect::<Vec<_>>();

    futures_util::future::join_all(futures).await
}

pub(crate) fn scored_chunk_to_json(chunk: &ScoredChunk) -> serde_json::Value {
    let is_page_raster = chunk.chunk_type == "page_raster"
        || chunk
            .parser_backend
            .as_deref()
            .is_some_and(|backend| backend.contains("visual_raster"));
    let mut value = serde_json::json!({
        "chunk_id": chunk.chunk_id.to_string(),
        "doc_id": chunk.doc_id.to_string(),
        "text": chunk.content,
        "score": chunk.score,
        "page": chunk.page,
        "source": chunk.source,
        "chunk_type": chunk.chunk_type,
        "parser_backend": chunk.parser_backend,
    });
    if is_page_raster {
        value["modality"] = serde_json::json!("page_raster");
        value["retrieval_hint"] =
            serde_json::json!("该片段来自页图向量，无 OCR 正文；引用请标注页码范围");
        if let Some(locator) = &chunk.source_locator {
            for key in ["page_range_start", "page_range_end", "page_numbers"] {
                if let Some(entry) = locator.get(key).and_then(|v| v.as_str()) {
                    value[key] = serde_json::json!(entry);
                }
            }
            if value.get("page_range_start").is_none() {
                if let Some(page) = locator.get("page").and_then(|v| v.as_u64()) {
                    value["page_range_start"] = serde_json::json!(page.to_string());
                    value["page_range_end"] = serde_json::json!(page.to_string());
                }
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn scored_chunk_to_json_includes_page_range_for_page_raster() {
        let chunk = ScoredChunk::new_text(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "page summary".to_string(),
            0.91,
            "multimodal_dense".to_string(),
            Some(5),
        )
        .with_metadata(
            "page_raster".to_string(),
            Some("visual_raster_pdf".to_string()),
            Some(serde_json::json!({
                "page": 5,
                "page_range_start": "5",
                "page_range_end": "8",
                "page_numbers": "5,6,7,8"
            })),
        );
        let value = scored_chunk_to_json(&chunk);
        assert_eq!(value["modality"], "page_raster");
        assert_eq!(value["page_range_start"], "5");
        assert_eq!(value["page_range_end"], "8");
        assert_eq!(value["page_numbers"], "5,6,7,8");
    }

    #[test]
    fn scored_chunk_to_json_falls_back_to_single_page_range() {
        let chunk = ScoredChunk::new_text(
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            "single page".to_string(),
            0.5,
            "multimodal_dense".to_string(),
            Some(12),
        )
        .with_metadata(
            "page_raster".to_string(),
            Some("visual_raster_pdf".to_string()),
            Some(serde_json::json!({ "page": 12 })),
        );
        let value = scored_chunk_to_json(&chunk);
        assert_eq!(value["page_range_start"], "12");
        assert_eq!(value["page_range_end"], "12");
    }
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
