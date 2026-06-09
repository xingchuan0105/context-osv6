use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::GraphSearchRequest;
use common::{GraphRetrievalArgs, ToolResult, ToolStatus, ToolTrace};
use uuid::Uuid;

use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: GraphRetrievalArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("graph_retrieval", format!("invalid args: {e}"));
        }
    };

    let entity_names: Vec<String> = args
        .graph_hints
        .iter()
        .flat_map(|h| [h.subject.clone(), h.object.clone()])
        .flatten()
        .collect();

    let relation_hints = args
        .graph_hints
        .into_iter()
        .map(|h| avrag_retrieval_data_plane::GraphRelationHint {
            subject: h.subject,
            predicate: h.predicate,
            object: h.object,
        })
        .chain(args.placeholder_triplets.into_iter().map(|t| {
            avrag_retrieval_data_plane::GraphRelationHint {
                subject: normalize_triplet_slot(&t.subject),
                predicate: normalize_triplet_slot(&t.predicate),
                object: normalize_triplet_slot(&t.object),
            }
        }))
        .collect();

    let doc_ids = if args.doc_scope.is_empty() {
        None
    } else {
        Some(
            args.doc_scope
                .iter()
                .filter_map(|id| Uuid::parse_str(id).ok())
                .collect(),
        )
    };

    let started = std::time::Instant::now();
    match runtime
        .data_plane
        .search_graph(GraphSearchRequest {
            auth: auth.clone(),
            doc_ids,
            entity_names,
            relation_hints,
            relation_limit: args.relation_limit,
            supporting_chunk_limit: args.supporting_chunk_limit,
            query_entities: Vec::new(),
            query_entity_vectors: Vec::new(),
            hop_limit: args.hop_limit,
            fan_out_limit: args.fan_out_limit,
            tenant_org_id: auth.org_id().to_string(),
        })
        .await
    {
        Ok(output) => {
            let mut supporting_chunks = output.supporting_chunks;

            // Rerank relation paths when a reranker is configured.
            if let Some(reranker) = runtime.reranker() {
                let query = args.query.as_deref().unwrap_or("");
                if !query.is_empty() && !output.relation_paths.is_empty() {
                    let path_texts: Vec<String> = output
                        .relation_paths
                        .iter()
                        .map(|p| {
                            format!(
                                "{} -{}-> {}",
                                if p.subject.trim().is_empty() {
                                    "?"
                                } else {
                                    &p.subject
                                },
                                if p.predicate.trim().is_empty() {
                                    "?"
                                } else {
                                    &p.predicate
                                },
                                if p.object.trim().is_empty() {
                                    "?"
                                } else {
                                    &p.object
                                },
                            )
                        })
                        .collect();

                    match reranker.rerank(query, &path_texts).await {
                        Ok(rerank_results) => {
                            // Build a map from original index to rerank score.
                            let mut scored_paths: Vec<(usize, f32, _)> = rerank_results
                                .into_iter()
                                .filter_map(|r| {
                                    output
                                        .relation_paths
                                        .get(r.index)
                                        .map(|path| (r.index, r.score, path.clone()))
                                })
                                .collect();
                            // Sort by rerank score descending.
                            scored_paths.sort_by(|a, b| {
                                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                            });
                            // Take top relation_limit paths.
                            let top_paths: Vec<_> = scored_paths
                                .into_iter()
                                .take(args.relation_limit)
                                .map(|(_, _, path)| path)
                                .collect();

                            // Collect supporting chunks referenced by top paths.
                            let top_chunk_ids: std::collections::HashSet<_> = top_paths
                                .iter()
                                .flat_map(|p| &p.supporting_chunk_ids)
                                .copied()
                                .collect();
                            supporting_chunks.retain(|c| top_chunk_ids.contains(&c.chunk_id));
                        }
                        Err(e) => {
                            // Fallback to original order on rerank failure.
                            tracing::warn!(error = %e, "graph relation rerank failed, using original order");
                        }
                    }
                }
            }

            let chunks: Vec<crate::ScoredChunk> = supporting_chunks;
            ToolResult {
                tool: "graph_retrieval".to_string(),
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
                    degrade_reason: None,
                }),
            }
        }
        Err(e) => super::error_result("graph_retrieval", e.to_string()),
    }
}

fn normalize_triplet_slot(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains('?') {
        None
    } else {
        Some(value.to_string())
    }
}
