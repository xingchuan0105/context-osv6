//! Local document reindex (G5): re-vectorize documents ingested while RAG was off.

use serde::Serialize;

use super::api::{api_call, IpcApiError};
use super::local_session::local_session_token;

#[derive(Debug, Clone, Serialize)]
pub struct ReindexDocumentsResult {
    pub total: usize,
    pub reindexed: usize,
    pub errors: Vec<String>,
}

/// The local API wraps list responses in `{ documents: [...] }` (direct) or
/// `{ data: { documents: [...] } }` (enveloped); accept either shape.
fn extract_document_ids(value: &serde_json::Value) -> Vec<String> {
    let docs = value
        .get("documents")
        .or_else(|| value.get("data").and_then(|d| d.get("documents")))
        .and_then(|d| d.as_array());
    docs.map(|arr| {
        arr.iter()
            .filter_map(|doc| doc.get("id").and_then(|id| id.as_str()))
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Reindex every local document via the local product API. Consumes embedding
/// tokens — this is the manual "重新索引本机文档" action (Q6: manual reindex).
#[tauri::command]
pub async fn reindex_local_documents(
    app: tauri::AppHandle,
) -> Result<ReindexDocumentsResult, IpcApiError> {
    let token = local_session_token(&app)
        .ok_or_else(|| IpcApiError::service_unavailable("no local session token"))?;

    let listed = api_call(
        "GET".into(),
        "/api/v1/documents".into(),
        None,
        Some(token.clone()),
    )
    .await?;
    let ids = extract_document_ids(&listed);

    let mut reindexed = 0usize;
    let mut errors = Vec::new();
    for id in &ids {
        match api_call(
            "POST".into(),
            format!("/api/v1/documents/{id}/reindex"),
            Some(serde_json::json!({})),
            Some(token.clone()),
        )
        .await
        {
            Ok(_) => reindexed += 1,
            Err(e) => errors.push(format!("{id}: {e}")),
        }
    }

    Ok(ReindexDocumentsResult {
        total: ids.len(),
        reindexed,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_ids_from_direct_and_enveloped_shapes() {
        let direct = json!({ "documents": [{ "id": "a" }, { "id": "b" }] });
        assert_eq!(extract_document_ids(&direct), vec!["a", "b"]);

        let enveloped = json!({ "data": { "documents": [{ "id": "c" }] } });
        assert_eq!(extract_document_ids(&enveloped), vec!["c"]);

        assert!(extract_document_ids(&json!({})).is_empty());
    }
}
