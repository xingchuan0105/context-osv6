use avrag_auth::AuthContext;
use contracts::{IndexLookupArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: IndexLookupArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("index_lookup", format!("invalid args: {e}"));
        }
    };

    if args.chunk_ids.is_empty() {
        return super::error_result("index_lookup", "chunk_ids must not be empty".to_string());
    }

    let filter_by_doc = !args.doc_id.trim().is_empty();
    let doc_id_filter = if filter_by_doc {
        match Uuid::parse_str(&args.doc_id) {
            Ok(id) => id,
            Err(e) => {
                return super::error_result("index_lookup", format!("invalid doc_id: {e}"));
            }
        }
    } else {
        Uuid::nil()
    };

    let chunk_uuids: Vec<Uuid> = args
        .chunk_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if chunk_uuids.is_empty() {
        return super::error_result("index_lookup", "no valid chunk_ids provided".to_string());
    }

    let content_store = match runtime.config.content_store.as_ref() {
        Some(store) => store,
        None => {
            return super::error_result(
                "index_lookup",
                "content_store is not configured".to_string(),
            );
        }
    };

    let started = std::time::Instant::now();
    match content_store.get_chunks_by_ids(auth, &chunk_uuids).await {
        Ok(chunks) => {
            let filtered: Vec<super::ScoredChunk> = chunks
                .values()
                .filter(|chunk| {
                    if !filter_by_doc {
                        return true;
                    }
                    chunk
                        .doc_id
                        .parse::<Uuid>()
                        .map(|doc_id| doc_id == doc_id_filter)
                        .unwrap_or(false)
                })
                .map(|chunk| {
                    let doc_id = if filter_by_doc {
                        doc_id_filter
                    } else {
                        chunk.doc_id.parse().unwrap_or_default()
                    };
                    super::ScoredChunk {
                        chunk_id: chunk.chunk_id.parse().unwrap_or_default(),
                        doc_id,
                        content: chunk.content.clone(),
                        score: chunk.score.unwrap_or(1.0),
                        source: "index_lookup".to_string(),
                        page: chunk.page,
                        chunk_type: "text".to_string(),
                        asset_id: None,
                        caption: None,
                        image_path: None,
                        parser_backend: None,
                        source_locator: None,
                        parse_run_id: None,
                    }
                })
                .collect();

            ToolResult {
                tool: "index_lookup".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(
                    serde_json::to_value(
                        filtered
                            .iter()
                            .map(super::scored_chunk_to_json)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap_or_default(),
                ),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(filtered.len()),
                    hydrated_hit_count: Some(filtered.len()),
                    degrade_reason: None,
                }),
            }
        }
        Err(e) => super::error_result("index_lookup", e.to_string()),
    }
}
