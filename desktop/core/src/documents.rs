//! 本机文档重索引（G5）:对 RAG 关闭期间入库的文档重新向量化。
//! REST 与鉴权都在 core;宿主负责提供本地会话 token。

use serde::Serialize;

use crate::api_proxy::api_call;
use crate::host_error::HostError;

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
pub async fn reindex_local_documents(token: &str) -> Result<ReindexDocumentsResult, HostError> {
    let listed = api_call(
        "GET".into(),
        "/api/v1/documents".into(),
        None,
        Some(token.to_string()),
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
            Some(token.to_string()),
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
