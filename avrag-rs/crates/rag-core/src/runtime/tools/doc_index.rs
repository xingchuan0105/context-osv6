use avrag_auth::AuthContext;
use avrag_storage_pg::TocEntry;
use common::{ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

#[derive(Debug, serde::Deserialize)]
struct DocIndexArgs {
    doc_ids: Vec<String>,
}

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: DocIndexArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("doc_index", format!("invalid args: {e}"));
        }
    };

    if args.doc_ids.is_empty() {
        return super::error_result("doc_index", "doc_ids must not be empty".to_string());
    }

    let doc_uuids: Vec<Uuid> = args
        .doc_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if doc_uuids.is_empty() {
        return super::error_result("doc_index", "no valid doc_ids provided".to_string());
    }

    let Some(pg_repo) = runtime.config.pg_repo.as_ref() else {
        return ToolResult {
            tool: "doc_index".to_string(),
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

    match pg_repo.get_document_toc_entries(auth, &doc_uuids).await {
        Ok(entries) => {
            let mut by_doc: std::collections::HashMap<Uuid, Vec<TocEntry>> =
                std::collections::HashMap::new();
            for (doc_id, entry) in entries {
                by_doc.entry(doc_id).or_default().push(entry);
            }

            let results: Vec<serde_json::Value> = doc_uuids
                .iter()
                .map(|doc_id| {
                    let Some(rows) = by_doc.get(doc_id) else {
                        return serde_json::json!({
                            "doc_id": doc_id.to_string(),
                            "error": {
                                "code": "DOC_NOT_FOUND",
                                "message": "Document not found or no index available."
                            }
                        });
                    };
                    if rows.is_empty() {
                        return serde_json::json!({
                            "doc_id": doc_id.to_string(),
                            "error": {
                                "code": "INDEX_EMPTY",
                                "message": "Document has no section index."
                            }
                        });
                    }
                    let index = aggregate_sections(rows);
                    serde_json::json!({
                        "doc_id": doc_id.to_string(),
                        "index": index,
                    })
                })
                .collect();

            let hit_count = results.len();
            ToolResult {
                tool: "doc_index".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(results)),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(hit_count),
                    hydrated_hit_count: Some(hit_count),
                    degrade_reason: None,
                }),
            }
        }
        Err(e) => super::error_result("doc_index", e.to_string()),
    }
}

fn aggregate_sections(rows: &[TocEntry]) -> Vec<serde_json::Value> {
    let mut sections: Vec<(String, i32, Option<i32>, i32, Vec<String>)> = Vec::new();

    for row in rows {
        let chunk_id = match row.chunk_id {
            Some(id) => id.to_string(),
            None => continue,
        };
        if let Some((title, level, page, rank, ids)) = sections.last_mut() {
            if *title == row.title && *level == row.heading_level && *rank == row.rank {
                ids.push(chunk_id);
                continue;
            }
            if *title == row.title && *level == row.heading_level {
                ids.push(chunk_id);
                continue;
            }
            let _ = (page, rank);
        }
        sections.push((
            row.title.clone(),
            row.heading_level,
            row.page,
            row.rank,
            vec![chunk_id],
        ));
    }

    sections
        .into_iter()
        .map(|(title, level, _page, _rank, chunk_ids)| {
            serde_json::json!({
                "title": title,
                "level": level,
                "chunk_ids": chunk_ids,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use avrag_storage_pg::TocEntry;

    #[test]
    fn aggregate_groups_same_title_and_level() {
        let rows = vec![
            TocEntry {
                id: Uuid::new_v4(),
                parent_id: None,
                title: "Intro".to_string(),
                heading_level: 1,
                page: None,
                chunk_id: Some(Uuid::new_v4()),
                rank: 0,
            },
            TocEntry {
                id: Uuid::new_v4(),
                parent_id: None,
                title: "Intro".to_string(),
                heading_level: 1,
                page: None,
                chunk_id: Some(Uuid::new_v4()),
                rank: 0,
            },
        ];
        let out = aggregate_sections(&rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["chunk_ids"].as_array().unwrap().len(), 2);
    }
}
