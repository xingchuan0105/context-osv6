use contracts::auth_runtime::AuthContext;
use contracts::{DocSummaryArgs, DocSummaryLevel, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: DocSummaryArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_summary", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_summary", "doc_ids must not be empty".to_string());
    }

    if matches!(args.level, DocSummaryLevel::Section) {
        return super::error_result(
            "doc_summary",
            "level=section is not supported; use doc_profile() for section titles and chunk_id mapping"
                .to_string(),
        );
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("doc_summary", "no valid doc_ids provided".to_string());
    }

    let Some(content_store) = runtime.config.content_store.as_ref() else {
        return ToolResult {
            tool: "doc_summary".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::Value::Array(Vec::new())),
            trace: Some(ToolTrace {
                elapsed_ms: Some(0),
                raw_hit_count: Some(0),
                hydrated_hit_count: Some(0),
                degrade_reason: Some("content_store not configured — returning empty".to_string()),
            }),
        };
    };

    let started = std::time::Instant::now();

    match content_store.get_summary_chunks(auth, &doc_uuids).await {
        Ok(summaries) => {
            let results: Vec<serde_json::Value> = summaries
                .into_iter()
                .map(|(doc_id, content)| {
                    serde_json::json!({
                        "doc_id": doc_id.to_string(),
                        "level": "doc",
                        "summary": content,
                    })
                })
                .collect();

            let hydrated_count = results.len();
            ToolResult {
                tool: "doc_summary".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(results)),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(doc_uuids.len()),
                    hydrated_hit_count: Some(hydrated_count),
                    degrade_reason: None,
                }),
            }
        }
        Err(e) => super::error_result("doc_summary", e.to_string()),
    }
}
