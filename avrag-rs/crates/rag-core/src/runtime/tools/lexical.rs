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
            turnstile_token: None,
    };

    let terms_for_augment = args.terms.clone();
    let terms_for_hint = args.terms.clone();
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
            let mut chunks: Vec<crate::ScoredChunk> =
                lists.into_iter().flat_map(|list| list.chunks).collect();
            // Preserve pure BM25 list for block-level three-way L-eval (dense∪bm25∪graph).
            let bm25_json: Vec<_> = chunks.iter().map(super::scored_chunk_to_json).collect();
            // Local lexical L-eval (bm25∪graph) so single-tool prints still see fused evidence.
            let graph_in_rrf = if cfg.l_eval_rrf && !graph_context.is_empty() {
                let (fused, n_g) = graph_augment::l_eval_rrf_fuse(chunks, &graph_context, 60);
                chunks = fused;
                n_g
            } else {
                0
            };
            let scores: Vec<f32> = chunks.iter().map(|c| c.score).collect();
            let adaptive = super::super::adaptive_k::adaptive_k(&scores);
            let longlist = chunks.clone();
            chunks.truncate(adaptive.k);
            chunks = crate::merge::finalize_evidence_package(
                runtime.content_store().as_deref(),
                auth,
                chunks,
                longlist,
            )
            .await;
            let graph_in_top15 = chunks
                .iter()
                .take(15)
                .filter(|c| c.source == "graph")
                .count();

            let chunk_json: Vec<_> = chunks.iter().map(super::scored_chunk_to_json).collect();
            // K1: always the object shape — carries the adaptive-k decision +
            // coaching hint; graph_context rides as before. Extractors
            // (store/progress/eval/observed-ids) tolerate both shapes.
            let mut data = json!({
                "chunks": chunk_json,
                "adaptive_k": adaptive.k,
                "score_shape": adaptive.shape.as_str(),
                "request_query": query,
                "request_terms": terms_for_hint,
            });
            if cfg.l_eval_rrf {
                data["l_eval_rrf"] = json!(true);
                data["bm25_chunks"] = json!(bm25_json);
                data["graph_chunk_in_rrf"] = json!(graph_in_rrf);
                data["graph_chunk_in_top15"] = json!(graph_in_top15);
            }
            if chunks.is_empty() {
                // 2026-07-29: 0 命中数据 hint——AND 语义下哪个词杀死了查询是
                // 服务端可确定的事实（按词命中数），直接给数据而非建议。
                let hint =
                    zero_hit_data_hint(runtime, auth, &terms_for_hint, &args.doc_scope).await;
                data["retrieval_hint"] = json!(hint);
            } else if let Some(hint) = super::super::adaptive_k::hint_text(&adaptive) {
                data["retrieval_hint"] = json!(hint);
            }
            if !graph_context.is_empty() {
                data["graph_context"] = json!(graph_context);
                data["graph_context_len"] = json!(graph_context.len());
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

/// 0 命中时按词计命中数（复用同一 bm25 通道，仅该路径触发，封顶 8 词）。
/// 返回 (词, 池内命中数)；池上限 100，达到即"≥100"语义。
async fn per_term_hit_counts(
    runtime: &RagRuntime,
    auth: &AuthContext,
    terms: &[String],
    doc_scope: &[String],
) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for term in terms.iter().take(8) {
        let request = ChatRequest {
            query: term.clone(),
            workspace_id: None,
            session_id: None,
            agent_type: "chat".to_string(),
            capabilities: None,
            client_context: None,
            client_ip: None,
            source_type: None,
            source_token: None,
            doc_scope: doc_scope.to_vec(),
            messages: Vec::new(),
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
            turnstile_token: None,
        };
        let plan = RagPlan {
            plan_version: "rag-item-v2".to_string(),
            plan_confidence: 1.0,
            clarify_needed: false,
            clarify_message: String::new(),
            items: vec![RagPlanItem {
                priority: 1.0,
                query: None,
                bm25_terms: Some(vec![term.clone()]),
                summary: None,
            }],
        };
        let n = runtime
            .retrieve_bm25_stage(&request, auth, &plan)
            .await
            .map(|(lists, _)| lists.iter().map(|l| l.chunks.len()).sum())
            .unwrap_or(0);
        out.push((term.clone(), n));
    }
    out
}

fn fmt_counts(counts: &[(String, usize)]) -> String {
    counts
        .iter()
        .map(|(t, n)| {
            if *n >= 100 {
                format!("「{t}」=≥100 条")
            } else {
                format!("「{t}」={n} 条")
            }
        })
        .collect::<Vec<_>>()
        .join("、")
}

/// 数据形态 0 命中 hint（2026-07-29，替代 E3 散文 hint）：多词 AND 下哪个词
/// 使整查归零是服务端可确定的事实——给词级命中数，删词动作由模型自读。
async fn zero_hit_data_hint(
    runtime: &RagRuntime,
    auth: &AuthContext,
    terms: &[String],
    doc_scope: &[String],
) -> String {
    if terms.len() == 1 {
        return format!(
            "0 命中：「{}」在 scope 内不存在。换词/换表述，或按查无流程处理。",
            terms[0]
        );
    }
    let counts = per_term_hit_counts(runtime, auth, terms, doc_scope).await;
    let detail = fmt_counts(&counts);
    let zeros: Vec<&str> = counts
        .iter()
        .filter(|(_, n)| *n == 0)
        .map(|(t, _)| t.as_str())
        .collect();
    if zeros.len() == counts.len() {
        format!(
            "0 命中：{detail}——全部词在 scope 内均不存在，按查无流程处理（不要换措辞重试同类查询）。"
        )
    } else if !zeros.is_empty() {
        format!(
            "0 命中（多词 AND）：{detail}——「{}」在 scope 内不存在；AND 语义下一词为零整查归零，删去 0 条词重试。",
            zeros.join("」「")
        )
    } else {
        format!("0 命中（多词 AND）：{detail}——各词分别存在但不共现于同一段；减词或拆成单词查询。")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_counts_marks_pool_ceiling() {
        let counts = vec![
            ("详细设计".to_string(), 3usize),
            ("阶段".to_string(), 100usize),
        ];
        let s = fmt_counts(&counts);
        assert!(s.contains("「详细设计」=3 条"), "{s}");
        assert!(s.contains("「阶段」=≥100 条"), "{s}");
    }
}
