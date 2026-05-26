use avrag_auth::AuthContext;
use common::{DocMetadataArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(
    runtime: &RagRuntime,
    auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let args: DocMetadataArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_metadata", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_metadata", "doc_ids must not be empty".to_string());
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("doc_metadata", "no valid doc_ids provided".to_string());
    }

    let Some(pg_repo) = runtime.config.pg_repo.as_ref() else {
        return ToolResult {
            tool: "doc_metadata".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::Value::Array(Vec::new())),
            trace: Some(ToolTrace {
                elapsed_ms: Some(0),
                raw_hit_count: Some(0),
                hydrated_hit_count: Some(0),
                degrade_reason: Some("pg_repo not configured — returning empty".to_string()),
            }),
        };
    };

    let started = std::time::Instant::now();

    let (metadata_list, toc_entries) = tokio::join!(
        pg_repo.get_document_metadata_by_ids(auth, &doc_uuids),
        pg_repo.get_document_toc_entries(auth, &doc_uuids),
    );

    match metadata_list {
        Ok(metadata_list) => {
            let toc_by_doc: std::collections::HashMap<String, Vec<serde_json::Value>> =
                toc_entries
                    .unwrap_or_default()
                    .into_iter()
                    .fold(std::collections::HashMap::new(), |mut acc, (doc_id, entry)| {
                        acc.entry(doc_id.to_string()).or_default().push(
                            serde_json::json!({
                                "title": entry.title,
                                "heading_level": entry.heading_level,
                                "page": entry.page,
                                "rank": entry.rank,
                            }),
                        );
                        acc
                    });

            let filtered: Vec<_> = if args.fields.is_empty() {
                metadata_list
                    .into_iter()
                    .map(|m| {
                        let mut obj = serde_json::json!({
                            "doc_id": m.doc_id,
                            "name": m.name,
                            "mime_type": m.mime_type,
                            "file_size": m.file_size,
                            "status": m.status.as_str(),
                            "chunk_count": m.chunk_count,
                        });
                        if let Some(toc) = toc_by_doc.get(&m.doc_id) {
                            obj["toc"] = serde_json::Value::Array(toc.clone());
                        }
                        obj
                    })
                    .collect()
            } else {
                metadata_list
                    .into_iter()
                    .map(|m| {
                        let mut obj = serde_json::Map::new();
                        obj.insert("doc_id".to_string(), serde_json::Value::String(m.doc_id.clone()));
                        for field in &args.fields {
                            match field.as_str() {
                                "name" => {
                                    obj.insert("name".to_string(), serde_json::Value::String(m.name.clone()));
                                }
                                "mime_type" => {
                                    obj.insert("mime_type".to_string(), serde_json::Value::String(m.mime_type.clone()));
                                }
                                "file_size" => {
                                    obj.insert("file_size".to_string(), serde_json::json!(m.file_size));
                                }
                                "status" => {
                                    obj.insert("status".to_string(), serde_json::Value::String(m.status.as_str().to_string()));
                                }
                                "chunk_count" => {
                                    obj.insert("chunk_count".to_string(), serde_json::json!(m.chunk_count));
                                }
                                "toc" => {
                                    if let Some(toc) = toc_by_doc.get(&m.doc_id) {
                                        obj.insert("toc".to_string(), serde_json::Value::Array(toc.clone()));
                                    }
                                }
                                _ => {}
                            }
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect()
            };

            let hydrated_count = filtered.len();
            ToolResult {
                tool: "doc_metadata".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(filtered)),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(doc_uuids.len()),
                    hydrated_hit_count: Some(hydrated_count),
                    degrade_reason: None,
                }),
            }
        }
        Err(e) => super::error_result("doc_metadata", e.to_string()),
    }
}
