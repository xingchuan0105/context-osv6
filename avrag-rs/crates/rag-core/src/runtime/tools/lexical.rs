use avrag_auth::AuthContext;
use common::{
    ChatRequest, LexicalRetrievalArgs, RagPlan, RagPlanItem, ToolResult, ToolStatus, ToolTrace,
};

use crate::RagRuntime;

pub async fn run(
    runtime: &RagRuntime,
    auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let args: LexicalRetrievalArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result("lexical_retrieval", format!("invalid args: {e}"));
        }
    };

    if args.terms.is_empty() {
        return super::error_result(
            "lexical_retrieval",
            "terms must not be empty".to_string(),
        );
    }

    let query = args.terms.join(" ");
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
    };

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
    match runtime
        .retrieve_bm25_stage(&request, auth, &rag_plan)
        .await
    {
        Ok((lists, degrade_trace)) => {
            let chunks: Vec<crate::ScoredChunk> =
                lists.into_iter().flat_map(|list| list.chunks).collect();
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::to_value(chunks.iter().map(super::scored_chunk_to_json).collect::<Vec<_>>()).unwrap_or_default()),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(started.elapsed().as_millis() as u64),
                    raw_hit_count: Some(chunks.len()),
                    hydrated_hit_count: Some(chunks.len()),
                    degrade_reason: if degrade_trace.is_empty() {
                        None
                    } else {
                        Some(degrade_trace.iter().map(|d| d.reason.as_str()).collect::<Vec<_>>().join("; "))
                    },
                }),
            }
        }
        Err(e) => super::error_result("lexical_retrieval", e.to_string()),
    }
}
