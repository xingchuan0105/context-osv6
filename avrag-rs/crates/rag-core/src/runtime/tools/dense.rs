use avrag_auth::AuthContext;
use common::{
    ChatRequest, DegradeTraceItem, DenseRetrievalArgs, LexicalRetrievalArgs, RagPlan, RagPlanItem,
    ToolResult, ToolStatus, ToolTrace,
};

use crate::RagRuntime;

pub(crate) fn embedding_failure_in_trace(degrade_trace: &[DegradeTraceItem]) -> bool {
    degrade_trace.iter().any(|item| {
        let reason = item.reason.to_ascii_lowercase();
        reason.contains("embedding failed") || reason.contains("embedding api error")
    })
}

pub(crate) fn lexical_args_from_dense(args: &DenseRetrievalArgs) -> serde_json::Value {
    let terms: Vec<String> = args
        .queries
        .iter()
        .flat_map(|query| {
            let words: Vec<String> = query
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect();
            if words.is_empty() {
                vec![query.clone()]
            } else {
                words
            }
        })
        .collect();

    serde_json::to_value(LexicalRetrievalArgs {
        terms,
        top_k: args.top_k,
        doc_scope: args.doc_scope.clone(),
    })
    .unwrap_or_default()
}

pub async fn run(
    runtime: &RagRuntime,
    auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let args: DenseRetrievalArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("dense_retrieval", format!("invalid args: {e}"));
        }
    };

    if args.queries.is_empty() {
        return super::error_result("dense_retrieval", "queries must not be empty".to_string());
    }

    let query = args.queries.join(" ");
    let request = ChatRequest {
        query: query.clone(),
        notebook_id: None,
        session_id: None,
        agent_type: "chat".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: args.doc_scope.clone(),
        messages: Vec::new(),
        stream: false,
        language: None,
        format_hint: None,
    };

    let items: Vec<RagPlanItem> = args
        .queries
        .iter()
        .enumerate()
        .map(|(idx, q)| RagPlanItem {
            priority: (1.0 - idx as f32 * 0.1).clamp(0.1, 1.0),
            query: Some(q.clone()),
            bm25_terms: None,
            summary: None,
        })
        .collect();

    let rag_plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items,
    };

    let started = std::time::Instant::now();
    match runtime
        .retrieve_text_dense_stage(&request, auth, &rag_plan)
        .await
    {
        Ok((lists, mut degrade_trace)) => {
            let mut chunks: Vec<crate::ScoredChunk> =
                lists.into_iter().flat_map(|list| list.chunks).collect();

            if chunks.is_empty()
                && (embedding_failure_in_trace(&degrade_trace) || !degrade_trace.is_empty())
            {
                if embedding_failure_in_trace(&degrade_trace) {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "dense_retrieval".to_string(),
                        reason: "embedding_unavailable".to_string(),
                        impact: "falling back to lexical_retrieval".to_string(),
                    });
                }

                let lexical_args = lexical_args_from_dense(&args);
                let lexical_result = super::lexical::run(runtime, auth, &lexical_args).await;
                if lexical_result.status == ToolStatus::Ok {
                    if let Some(items) = lexical_result
                        .data
                        .as_ref()
                        .and_then(|data| data.as_array())
                    {
                        chunks.reserve(items.len());
                        for item in items {
                            if let (Some(chunk_id), Some(doc_id), Some(text)) = (
                                item.get("chunk_id").and_then(|v| v.as_str()),
                                item.get("doc_id").and_then(|v| v.as_str()),
                                item.get("text").and_then(|v| v.as_str()),
                            ) {
                                chunks.push(crate::ScoredChunk::new_text(
                                    chunk_id.parse().unwrap_or_default(),
                                    doc_id.parse().unwrap_or_default(),
                                    text.to_string(),
                                    item.get("score")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0) as f32,
                                    item.get("source")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    item.get("page").and_then(|v| v.as_i64()),
                                ));
                            }
                        }
                    }
                }
            }

            let degrade_reason = if degrade_trace.is_empty() {
                None
            } else {
                Some(
                    degrade_trace
                        .iter()
                        .map(|d| d.reason.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            };

            ToolResult {
                tool: "dense_retrieval".to_string(),
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
                    degrade_reason,
                }),
            }
        }
        Err(e) => super::error_result("dense_retrieval", e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_failure_in_trace_detects_dense_embedding_errors() {
        let trace = vec![DegradeTraceItem {
            stage: "text_dense".to_string(),
            reason: "Text dense embedding failed: Embedding API error 503".to_string(),
            impact: "skip".to_string(),
        }];
        assert!(embedding_failure_in_trace(&trace));
    }

    #[test]
    fn embedding_failure_in_trace_ignores_unrelated_degrade() {
        let trace = vec![DegradeTraceItem {
            stage: "bm25".to_string(),
            reason: "BM25 channel failed".to_string(),
            impact: "skip".to_string(),
        }];
        assert!(!embedding_failure_in_trace(&trace));
    }

    #[test]
    fn lexical_args_from_dense_splits_query_terms() {
        let args = DenseRetrievalArgs {
            queries: vec!["What is antifragility?".to_string()],
            modality: common::DenseRetrievalModality::Text,
            top_k: 5,
            doc_scope: vec!["doc-1".to_string()],
        };
        let value = lexical_args_from_dense(&args);
        let terms = value["terms"].as_array().expect("terms array");
        assert!(terms.iter().any(|t| t.as_str() == Some("antifragility?")));
        assert_eq!(value["top_k"], 5);
        assert_eq!(value["doc_scope"][0], "doc-1");
    }
}
