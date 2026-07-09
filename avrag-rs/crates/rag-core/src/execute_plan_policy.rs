//! Runtime policy for `ExecutePlanRequest` — lives outside the contracts crate.
//!
//! Wire shapes stay in `contracts::rag_execute`. Call these helpers from rag-core /
//! app-chat instead of putting orchestration on the contract types.

use contracts::chat::{ChatRequest, RagPlan, RagPlanItem};
use contracts::rag_execute::PlaceholderTripletType;
use contracts::{
    ExecutePlanItem, ExecutePlanRequest, ExecutePlanSummaryMode, ExecutePlanValidationError,
    PlaceholderTriplet,
};

pub const MAX_EXECUTE_PLAN_ITEMS: usize = ExecutePlanRequest::MAX_ITEMS;

/// Classify a placeholder triplet for graph retrieval strategy selection.
pub fn classify_placeholder_triplet(triplet: &PlaceholderTriplet) -> PlaceholderTripletType {
    triplet.classify()
}

/// Wire + budget validation for an execute-plan request.
pub fn validate_execute_plan(req: &ExecutePlanRequest) -> Result<(), ExecutePlanValidationError> {
    req.validate()
}

/// Ensure the original user query is present as a text-dense item (retrieval prep).
pub fn ensure_original_query_text_dense_item(req: &mut ExecutePlanRequest, original_query: &str) {
    insert_original_query_item(&mut req.items, original_query);
}

/// Build a minimal ChatRequest for rag-core paths that still speak ChatRequest.
pub fn execute_plan_to_chat_request(req: &ExecutePlanRequest) -> ChatRequest {
    let query = req
        .items
        .iter()
        .find_map(|item| {
            item.query.clone().or_else(|| {
                item.bm25_terms
                    .as_ref()
                    .filter(|terms| !terms.is_empty())
                    .map(|terms| terms.join(" "))
            })
        })
        .unwrap_or_default();

    ChatRequest {
        query,
        notebook_id: None,
        session_id: None,
        agent_type: "rag".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: req.doc_scope.clone(),
        language: None,
        messages: Vec::new(),
        stream: false,
        debug: false,
        format_hint: None,
    }
}

/// Legacy RagPlan projection for transitional callers.
pub fn execute_plan_to_rag_plan(req: &ExecutePlanRequest) -> RagPlan {
    let mut items = req
        .items
        .iter()
        .map(|item| RagPlanItem {
            priority: item.priority,
            query: item.query.clone(),
            bm25_terms: item.bm25_terms.clone(),
            summary: None,
        })
        .collect::<Vec<_>>();
    if req.summary_mode != ExecutePlanSummaryMode::None {
        items.push(RagPlanItem {
            priority: 0.0,
            query: None,
            bm25_terms: None,
            summary: Some(req.summary_mode.as_str().to_string()),
        });
    }
    RagPlan {
        plan_version: req.plan_version.clone(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items,
    }
}

/// Build execute plan from a legacy RagPlan (kept near policy for symmetry).
pub fn execute_plan_from_rag_plan(plan: &RagPlan, doc_scope: &[String]) -> ExecutePlanRequest {
    ExecutePlanRequest::from_rag_plan(plan, doc_scope)
}

/// Prefer this over calling methods on the contract type in runtime code.
pub fn insert_original_query_item(items: &mut Vec<ExecutePlanItem>, original_query: &str) {
    let original_query = original_query.trim();
    if original_query.is_empty() {
        return;
    }
    if items.iter().any(|item| {
        item.query
            .as_deref()
            .is_some_and(|query| query.trim() == original_query)
    }) {
        return;
    }
    items.insert(
        0,
        ExecutePlanItem {
            priority: 1.0,
            query: Some(original_query.to_string()),
            bm25_terms: None,
        },
    );
    while items.len() > MAX_EXECUTE_PLAN_ITEMS {
        items.pop();
    }
}
