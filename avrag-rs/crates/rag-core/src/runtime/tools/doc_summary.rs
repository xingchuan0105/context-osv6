//! Joint document archive: metadata + summary + section tree (design 2026-08-06).
//! Replaces the former split of `doc_summary` + `doc_profile`.

use common::TocEntry;
use contracts::auth_runtime::AuthContext;
use contracts::{DocSummaryArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let mut normalized = args.clone();
    contracts::normalize_doc_id_alias(&mut normalized);
    // Accept legacy `{ "level": "doc" }` without failing.
    if let Some(obj) = normalized.as_object_mut() {
        obj.remove("level");
        obj.remove("fields");
    }
    let args: DocSummaryArgs = match serde_json::from_value(normalized) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_summary", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_summary", "doc_ids must not be empty".to_string());
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
            version: "2.0".to_string(),
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

    let (metadata_list, summary_meta, toc_entries, summary_chunks) = tokio::join!(
        content_store.get_document_metadata_by_ids(auth, &doc_uuids),
        content_store.get_summary_metadata(auth, &doc_uuids),
        content_store.get_document_toc_entries(auth, &doc_uuids),
        content_store.get_summary_chunks(auth, &doc_uuids),
    );

    let metadata_list = match metadata_list {
        Ok(m) => m,
        Err(e) => return super::error_result("doc_summary", e.to_string()),
    };

    let summary_by_doc: std::collections::HashMap<String, common::SummaryMetadata> = summary_meta
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.doc_id.clone(), m))
        .collect();

    let summary_text_by_doc: std::collections::HashMap<Uuid, String> = summary_chunks
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut toc_by_doc: std::collections::HashMap<String, Vec<TocEntry>> =
        std::collections::HashMap::new();
    for (doc_id, entry) in toc_entries.unwrap_or_default() {
        toc_by_doc
            .entry(doc_id.to_string())
            .or_default()
            .push(entry);
    }

    let results: Vec<serde_json::Value> = metadata_list
        .into_iter()
        .map(|m| {
            let summary_meta = summary_by_doc.get(&m.doc_id);
            let doc_uuid = Uuid::parse_str(&m.doc_id).ok();
            let summary_text = doc_uuid
                .and_then(|id| summary_text_by_doc.get(&id).cloned())
                .unwrap_or_default();

            let sections = toc_by_doc
                .get(&m.doc_id)
                .map(|rows| {
                    rows.iter()
                        .map(|entry| {
                            serde_json::json!({
                                "title": entry.title,
                                "heading_level": entry.heading_level,
                                "page": entry.page,
                                "rank": entry.rank,
                                "overview": entry.overview,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            serde_json::json!({
                "doc_id": m.doc_id,
                "metadata": {
                    "name": summary_meta.map(|s| s.docname.clone()).unwrap_or_else(|| m.name.clone()),
                    "author": summary_meta.and_then(|s| s.author.clone()),
                    "publication_date": summary_meta.and_then(|s| s.publication_date.clone()),
                    "language": summary_meta.map(|s| s.language.clone()).unwrap_or_else(|| "unknown".to_string()),
                    "domain": summary_meta.map(|s| s.domain.as_str().to_string()).unwrap_or_else(|| "unknown".to_string()),
                    "genre": summary_meta.map(|s| s.genre.as_str().to_string()).unwrap_or_else(|| "unknown".to_string()),
                    "era": summary_meta.map(|s| s.era.as_str().to_string()).unwrap_or_else(|| "unknown".to_string()),
                },
                "summary": summary_text,
                "sections": sections,
            })
        })
        .collect();

    let hydrated_count = results.len();
    ToolResult {
        tool: "doc_summary".to_string(),
        version: "2.0".to_string(),
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
