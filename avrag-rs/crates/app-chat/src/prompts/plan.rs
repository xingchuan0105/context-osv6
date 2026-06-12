use common::{ExecutePlanItem, ExecutePlanRequest, ExecutePlanSummaryMode, RetrievalPlannerOutput, ToolCall};
use contracts::chat::{ChatRequest};

use super::internal::{
    build_rag_envelope, extract_json_object, normalize_execute_plan_item, normalize_graph_hints,
    normalize_placeholder_triplets, normalize_query_entities, RAG_EXECUTE_PLAN_VERSION,
};
use super::types::{PlanStrategy, PlanStrategyItem, RagBehaviorSkill, RagContext, RagPlanDecision};

pub(crate) fn fallback_execute_plan_request(
    request: &ChatRequest,
    docscope_metadata: Option<&common::DocScopeMetadata>,
) -> ExecutePlanRequest {
    ExecutePlanRequest {
        plan_version: RAG_EXECUTE_PLAN_VERSION.to_string(),
        doc_scope: request.doc_scope.clone(),
        items: vec![ExecutePlanItem {
            priority: 1.0,
            query: Some(request.query.trim().to_string()),
            bm25_terms: None,
        }],
        summary_mode: if docscope_metadata.is_some_and(|metadata| !metadata.documents.is_empty()) {
            ExecutePlanSummaryMode::Related
        } else {
            ExecutePlanSummaryMode::None
        },
        budget: None,
        channel_budget: None,
        query_entities: Vec::new(),
        graph_hints: Vec::new(),
        placeholder_triplets: Vec::new(),
        trace: None,
    }
}

pub(crate) fn build_rag_plan_user_prompt(
    request: &ChatRequest,
    docscope_metadata: Option<&common::DocScopeMetadata>,
    previous_tool_results: &[common::ToolResult],
) -> String {
    let metadata_json = docscope_metadata
        .and_then(|metadata| serde_json::to_string_pretty(metadata).ok())
        .unwrap_or_else(|| "null".to_string());
    let doc_scope_json =
        serde_json::to_string(&request.doc_scope).unwrap_or_else(|_| "[]".to_string());
    let mut authoritative = format!(
        "Provided doc_scope JSON:\n{}\n\nDocscope metadata JSON:\n{}",
        doc_scope_json, metadata_json
    );

    // Inject previously retrieved doc_profile results so the planner can issue index_lookup.
    let doc_profile_results: Vec<&serde_json::Value> = previous_tool_results
        .iter()
        .filter(|r| r.tool == "doc_profile" && r.status == common::ToolStatus::Ok)
        .filter_map(|r| r.data.as_ref())
        .collect();
    if !doc_profile_results.is_empty() {
        let profile_json =
            serde_json::to_string_pretty(&doc_profile_results).unwrap_or_else(|_| "[]".to_string());
        authoritative.push_str(&format!(
            "\n\nDocument profile already retrieved (from prior iteration):\n{}\n\n\
             Based on section chunk_ids, call index_lookup for the sections needed to answer the user.",
            profile_json
        ));
    }

    build_rag_envelope(RagContext {
        mode: "rag-plan".to_string(),
        current_task: request.query.trim().to_string(),
        authoritative_context: authoritative,
        reference_context: "none".to_string(),
        user_preference_memory: "none".to_string(),
        skill: RagBehaviorSkill::new(
            "rag-plan",
            [
                "Generate an execute-plan for the RAG API.",
                "Return a strategy with retrieval tool calls for every non-empty user query.",
                "Use clarify ONLY when the user query is empty, meaningless, or the provided doc_scope contains no relevant documents.",
            ],
        ),
        output_contract: "Return exactly one raw JSON object: PlanStrategy ({\"strategy\":[{tool, param1, param2}],\"next_step\":\"answer\"}). Use {\"action\":\"clarify\",\"message\":\"...\"} ONLY when the query is empty or doc_scope is completely irrelevant.".to_string(),
    })
}

pub(crate) fn parse_rag_plan_decision(
    raw: &str,
    request: &ChatRequest,
) -> Option<(RagPlanDecision, Vec<String>)> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());

    // 1. Clarification object (either format)
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json)
        && value
            .get("action")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|action| action.eq_ignore_ascii_case("clarify"))
    {
        let message = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())?;
        // v5: fallback to default retrieval strategy when query and doc_scope are valid.
        // Prevents over-eager clarify from models that default to asking questions.
        if !request.query.trim().is_empty() && !request.doc_scope.is_empty() {
            return Some((
                RagPlanDecision::Strategy(PlanStrategy {
                    strategy: vec![PlanStrategyItem {
                        tool: "dense_retrieval".to_string(),
                        params: serde_json::json!({
                            "queries": vec![request.query.clone()],
                            "modality": "text",
                            "top_k": 10,
                        }),
                    }],
                    next_step: "answer".to_string(),
                }),
                Vec::new(),
            ));
        }
        return Some((RagPlanDecision::Clarify(message.to_string()), Vec::new()));
    }

    // 2. P4 format: PlanStrategy (plan-only, no schema-compliant args yet)
    if let Ok(strategy) = serde_json::from_str::<PlanStrategy>(&json)
        && !strategy.strategy.is_empty()
    {
        return Some((RagPlanDecision::Strategy(strategy), Vec::new()));
    }

    // 3. Phase-3c format: RetrievalPlannerOutput (ToolCall[])
    if let Ok(planner_output) = serde_json::from_str::<RetrievalPlannerOutput>(&json)
        && !planner_output.calls.is_empty()
    {
        // Phase-3c: bypass adapter — return raw ToolCalls for the dispatcher
        return Some((
            RagPlanDecision::ToolCalls(planner_output.calls),
            planner_output.skills,
        ));
    }

    // 4. Legacy format: ExecutePlanRequest (backward compatibility)
    let plan = serde_json::from_str::<ExecutePlanRequest>(&json).ok()?;
    if plan.validate().is_err() || plan.doc_scope != request.doc_scope {
        return None;
    }
    match normalize_execute_plan_request(plan, request) {
        Some(plan) => Some((
            RagPlanDecision::ToolCalls(execute_plan_request_to_tool_calls(plan)),
            Vec::new(),
        )),
        None => Some((
            RagPlanDecision::Clarify(
                crate::i18n::clarify::need_query_or_doc_scope(request.language.as_deref())
                    .to_string(),
            ),
            Vec::new(),
        )),
    }
}

/// Convert a `PlanStrategy` (plan-only format) directly into fully-formed `ToolCall`s.
/// Eliminates the need for a separate execute-phase LLM call.
pub(crate) fn plan_strategy_to_tool_calls(strategy: &PlanStrategy) -> Vec<ToolCall> {
    strategy
        .strategy
        .iter()
        .map(|item| ToolCall {
            tool: item.tool.clone(),
            version: "1.0".to_string(),
            args: item.params.clone(),
        })
        .collect()
}

/// Convert a legacy `ExecutePlanRequest` into the modern `Vec<ToolCall>` representation.
pub(crate) fn execute_plan_request_to_tool_calls(plan: ExecutePlanRequest) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    // Each item becomes either a dense_retrieval or lexical_retrieval call.
    for item in plan.items {
        if let Some(query) = item.query {
            calls.push(ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "queries": vec![query],
                    "modality": "text",
                    "top_k": 10,
                }),
            });
        } else if let Some(terms) = item.bm25_terms {
            calls.push(ToolCall {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "terms": terms,
                    "top_k": 10,
                }),
            });
        }
    }

    // Graph hints & placeholder triplets → graph_retrieval
    if !plan.graph_hints.is_empty() || !plan.placeholder_triplets.is_empty() {
        calls.push(ToolCall {
            tool: "graph_retrieval".to_string(),
            version: "1.0".to_string(),
            args: serde_json::json!({
                "graph_hints": plan.graph_hints,
                "placeholder_triplets": plan.placeholder_triplets,
                "relation_limit": 20,
                "supporting_chunk_limit": 10,
            }),
        });
    }

    // Summary mode → doc_summary
    match plan.summary_mode {
        common::ExecutePlanSummaryMode::All => {
            calls.push(ToolCall {
                tool: "doc_summary".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "doc_ids": plan.doc_scope.clone(),
                    "level": "doc",
                }),
            });
        }
        common::ExecutePlanSummaryMode::Related => {
            calls.push(ToolCall {
                tool: "doc_summary".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "doc_ids": plan.doc_scope.clone(),
                    "level": "section",
                }),
            });
        }
        common::ExecutePlanSummaryMode::None => {}
    }

    calls
}

pub(crate) fn normalize_execute_plan_request(
    mut plan: ExecutePlanRequest,
    request: &ChatRequest,
) -> Option<ExecutePlanRequest> {
    if plan.plan_version.trim().is_empty() {
        plan.plan_version = RAG_EXECUTE_PLAN_VERSION.to_string();
    }
    plan.doc_scope = request.doc_scope.clone();
    plan.trace = None;
    plan.items = plan
        .items
        .into_iter()
        .filter_map(normalize_execute_plan_item)
        .take(4)
        .collect();
    plan.query_entities = normalize_query_entities(plan.query_entities);
    plan.graph_hints = normalize_graph_hints(plan.graph_hints);
    plan.placeholder_triplets = normalize_placeholder_triplets(plan.placeholder_triplets);
    plan.validate().ok()?;
    Some(plan)
}

