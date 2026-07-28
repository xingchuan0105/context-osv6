use contracts::auth_runtime::AuthContext;
use contracts::chat::{ChatRequest, RagPlan, RagPlanItem};
use contracts::{LexicalRetrievalArgs, ToolResult, ToolStatus, ToolTrace};
use serde_json::json;

use super::graph_augment::{self, GraphAugmentConfig};
use crate::RagRuntime;

pub async fn run(runtime: &RagRuntime, auth: &AuthContext, args: &serde_json::Value) -> ToolResult {
    let args: LexicalRetrievalArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("lexical_retrieval", format!("invalid args: {e}"));
        }
    };

    if args.terms.is_empty() {
        return super::error_result("lexical_retrieval", "terms must not be empty".to_string());
    }

    let query = args.terms.join(" ");
    let request = ChatRequest {
        query: query.clone(),
        workspace_id: None,
        session_id: None,
        agent_type: "chat".to_string(),
        capabilities: None,
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: args.doc_scope.clone(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    };

    let terms_for_augment = args.terms.clone();
    let rag_plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 1.0,
            query: None,
            bm25_terms: Some(args.terms),
            summary: None,
        }],
    };

    let started = std::time::Instant::now();
    let cfg = GraphAugmentConfig::resolve();
    let augment_fut = async {
        if cfg.enabled {
            graph_augment::graph_augment_from_terms(
                runtime,
                auth,
                &terms_for_augment,
                &args.doc_scope,
                &cfg,
            )
            .await
        } else {
            Vec::new()
        }
    };

    let (bm25_result, graph_context) = tokio::join!(
        runtime.retrieve_bm25_stage(&request, auth, &rag_plan),
        augment_fut
    );

    match bm25_result {
        Ok((lists, degrade_trace)) => {
            let chunks: Vec<crate::ScoredChunk> =
                lists.into_iter().flat_map(|list| list.chunks).collect();
            // K1: adaptive top-k on the similarity scores (no rerank here —
            // the retrieval order is the display order).
            let scores: Vec<f32> = chunks.iter().map(|c| c.score).collect();
            let adaptive = super::super::adaptive_k::adaptive_k(&scores);
            let mut chunks = chunks;
            chunks.truncate(adaptive.k);
            let chunk_json: Vec<_> = chunks
                .iter()
                .map(super::scored_chunk_to_json)
                .collect();
            // K1: always the object shape — carries the adaptive-k decision +
            // coaching hint; graph_context rides as before. Extractors
            // (store/progress/eval/observed-ids) tolerate both shapes.
            let mut data = json!({
                "chunks": chunk_json,
                "adaptive_k": adaptive.k,
                "score_shape": adaptive.shape.as_str(),
            });
            if let Some(hint) = super::super::adaptive_k::hint_text(&adaptive) {
                data["retrieval_hint"] = json!(hint);
            }
            if !graph_context.is_empty() {
                data["graph_context"] = json!(graph_context);
            }
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(data),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(chunks.len()),
                    hydrated_hit_count: Some(chunks.len()),
                    degrade_reason: if degrade_trace.is_empty() {
                        None
                    } else {
                        Some(
                            degrade_trace
                                .iter()
                                .map(|d| d.reason.as_str())
                                .collect::<Vec<_>>()
                                .join("; "),
                        )
                    },
                }),
            }
        }
        Err(e) => super::error_result("lexical_retrieval", e.to_string()),
    }
}
