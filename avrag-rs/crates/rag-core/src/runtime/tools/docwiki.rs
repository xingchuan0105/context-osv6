//! Document **docwiki**: plain-text archive (metadata + overview + structure as prose).

use contracts::auth_runtime::AuthContext;
use contracts::{DocWikiArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let mut normalized = args.clone();
    contracts::normalize_doc_id_alias(&mut normalized);
    // Drop retired structured fields if a caller still sends them.
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("level");
        obj.remove("fields");
    }
    let args: DocWikiArgs = match serde_json::from_value(normalized) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("docwiki", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("docwiki", "doc_ids must not be empty".to_string());
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("docwiki", "no valid doc_ids provided".to_string());
    }

    let Some(content_store) = runtime.config.content_store.as_ref() else {
        return ToolResult {
            tool: "docwiki".to_string(),
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

    let (metadata_list, docwiki_chunks) = tokio::join!(
        content_store.get_document_metadata_by_ids(auth, &doc_uuids),
        content_store.get_docwiki_chunks(auth, &doc_uuids),
    );

    let metadata_list = match metadata_list {
        Ok(m) => m,
        Err(e) => return super::error_result("docwiki", e.to_string()),
    };

    let text_by_doc: std::collections::HashMap<Uuid, String> = docwiki_chunks
        .unwrap_or_default()
        .into_iter()
        .collect();

    let results: Vec<serde_json::Value> = metadata_list
        .into_iter()
        .map(|m| {
            let doc_uuid = Uuid::parse_str(&m.doc_id).ok();
            let content = doc_uuid
                .and_then(|id| text_by_doc.get(&id).cloned())
                .unwrap_or_default();
            serde_json::json!({
                "doc_id": m.doc_id,
                "name": m.name,
                "content": content,
            })
        })
        .collect();

    let hydrated_count = results.len();
    ToolResult {
        tool: "docwiki".to_string(),
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
