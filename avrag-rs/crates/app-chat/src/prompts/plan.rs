use contracts::chat::ChatRequest;
use contracts::{RetrievalPlannerOutput, ToolCall};

use super::internal::{build_rag_envelope, extract_json_object};
use super::types::{PlanStrategy, PlanStrategyItem, RagBehaviorSkill, RagContext, RagPlanDecision};

pub(crate) fn build_rag_plan_user_prompt(
    request: &ChatRequest,
    docscope_metadata: Option<&common::DocScopeMetadata>,
    previous_tool_results: &[contracts::ToolResult],
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
        .filter(|r| r.tool == "doc_profile" && r.status == contracts::ToolStatus::Ok)
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
                "Generate retrieval tool calls for the RAG agent loop (AgentLoop + ToolCall).",
                "Return a strategy with retrieval tool calls for every non-empty user query.",
                "Use clarify ONLY when the user query is empty, meaningless, or the provided doc_scope contains no relevant documents.",
            ],
        ),
        output_contract: "Return exactly one raw JSON object: PlanStrategy ({\"strategy\":[{tool, param1, param2}],\"next_step\":\"answer\"}) or RetrievalPlannerOutput ({\"calls\":[...],\"skills\":[],\"next_step\":\"answer\"}). Use {\"action\":\"clarify\",\"message\":\"...\"} ONLY when the query is empty or doc_scope is completely irrelevant. Do not emit legacy ExecutePlanRequest JSON.".to_string(),
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
        // Fallback to default retrieval strategy when query and doc_scope are valid.
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

    // 2. PlanStrategy (plan-only, no schema-compliant args yet)
    if let Ok(strategy) = serde_json::from_str::<PlanStrategy>(&json)
        && !strategy.strategy.is_empty()
    {
        return Some((RagPlanDecision::Strategy(strategy), Vec::new()));
    }

    // 3. RetrievalPlannerOutput (ToolCall[])
    // ADR-0006 / TN Wave 2: ExecutePlanRequest is not accepted. Product path = ToolCall only.
    if let Ok(planner_output) = serde_json::from_str::<RetrievalPlannerOutput>(&json)
        && !planner_output.calls.is_empty()
    {
        return Some((
            RagPlanDecision::ToolCalls(planner_output.calls),
            planner_output.skills,
        ));
    }

    None
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
