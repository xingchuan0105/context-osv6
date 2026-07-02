use avrag_auth::AuthContext;
use common::TocEntry;
use contracts::{DocProfileArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: DocProfileArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_profile", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_profile", "doc_ids must not be empty".to_string());
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("doc_profile", "no valid doc_ids provided".to_string());
    }

    let Some(content_store) = runtime.config.content_store.as_ref() else {
        return ToolResult {
            tool: "doc_profile".to_string(),
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

    let (metadata_list, summary_meta, toc_entries) = tokio::join!(
        content_store.get_document_metadata_by_ids(auth, &doc_uuids),
        content_store.get_summary_metadata(auth, &doc_uuids),
        content_store.get_document_toc_entries(auth, &doc_uuids),
    );

    match metadata_list {
        Ok(metadata_list) => {
            let summary_by_doc: std::collections::HashMap<String, common::SummaryMetadata> =
                summary_meta
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| (m.doc_id.clone(), m))
                    .collect();

            let mut toc_by_doc: std::collections::HashMap<String, Vec<TocEntry>> =
                std::collections::HashMap::new();
            for (doc_id, entry) in toc_entries.unwrap_or_default() {
                toc_by_doc
                    .entry(doc_id.to_string())
                    .or_default()
                    .push(entry);
            }

            let include =
                |field: &str| args.fields.is_empty() || args.fields.iter().any(|f| f == field);

            let filtered: Vec<_> = metadata_list
                .into_iter()
                .map(|m| {
                    let summary = summary_by_doc.get(&m.doc_id);
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "doc_id".to_string(),
                        serde_json::Value::String(m.doc_id.clone()),
                    );

                    if include("name") {
                        obj.insert(
                            "name".to_string(),
                            serde_json::Value::String(
                                summary
                                    .map(|s| s.docname.clone())
                                    .unwrap_or_else(|| m.name.clone()),
                            ),
                        );
                    }
                    if include("author") {
                        obj.insert(
                            "author".to_string(),
                            summary
                                .and_then(|s| s.author.clone())
                                .map(serde_json::Value::String)
                                .unwrap_or(serde_json::Value::Null),
                        );
                    }
                    if include("publication_date") {
                        obj.insert(
                            "publication_date".to_string(),
                            summary
                                .and_then(|s| s.publication_date.clone())
                                .map(serde_json::Value::String)
                                .unwrap_or(serde_json::Value::Null),
                        );
                    }
                    if include("language") {
                        obj.insert(
                            "language".to_string(),
                            serde_json::Value::String(
                                summary
                                    .map(|s| s.language.clone())
                                    .unwrap_or_else(|| "unknown".to_string()),
                            ),
                        );
                    }
                    if include("domain") {
                        obj.insert(
                            "domain".to_string(),
                            serde_json::Value::String(
                                summary
                                    .map(|s| s.domain.as_str().to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                            ),
                        );
                    }
                    if include("genre") {
                        obj.insert(
                            "genre".to_string(),
                            serde_json::Value::String(
                                summary
                                    .map(|s| s.genre.as_str().to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                            ),
                        );
                    }
                    if include("era") {
                        obj.insert(
                            "era".to_string(),
                            serde_json::Value::String(
                                summary
                                    .map(|s| s.era.as_str().to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                            ),
                        );
                    }
                    if include("toc") || include("sections") {
                        if let Some(rows) = toc_by_doc.get(&m.doc_id) {
                            let sections: Vec<serde_json::Value> = rows
                                .iter()
                                .map(|entry| {
                                    serde_json::json!({
                                        "title": entry.title,
                                        "heading_level": entry.heading_level,
                                        "page": entry.page,
                                        "rank": entry.rank,
                                        "chunk_id": entry.chunk_id.map(|id| id.to_string()),
                                    })
                                })
                                .collect();
                            obj.insert("sections".to_string(), serde_json::Value::Array(sections));
                        }
                    }

                    serde_json::Value::Object(obj)
                })
                .collect();

            let hydrated_count = filtered.len();
            ToolResult {
                tool: "doc_profile".to_string(),
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
        Err(e) => super::error_result("doc_profile", e.to_string()),
    }
}
