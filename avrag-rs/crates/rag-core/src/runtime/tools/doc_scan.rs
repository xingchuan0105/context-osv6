use contracts::auth_runtime::AuthContext;
use contracts::{DocChunksArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

const MAX_SCAN_CHUNKS: usize = 16384;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: DocChunksArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_scan", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_scan", "doc_ids must not be empty".to_string());
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("doc_scan", "no valid doc_ids provided".to_string());
    }

    let started = std::time::Instant::now();
    match runtime.list_text_chunks(auth, &doc_uuids).await {
        Ok(chunks) => {
            if chunks.len() > MAX_SCAN_CHUNKS {
                return super::error_result(
                    "doc_scan",
                    format!(
                        "chunk count {} exceeds limit {}; narrow doc_scope",
                        chunks.len(),
                        MAX_SCAN_CHUNKS
                    ),
                );
            }

            ToolResult {
                tool: "doc_scan".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(
                    serde_json::to_value(
                        chunks
                            .iter()
                            .map(super::scored_chunk_to_json)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap_or_default(),
                ),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(chunks.len()),
                    hydrated_hit_count: Some(chunks.len()),
                    degrade_reason: Some("scan_data".to_string()),
                }),
            }
        }
        Err(e) => super::error_result("doc_scan", e.to_string()),
    }
}
