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
    let placeholder_count = triplet.subject.starts_with('?') as usize
        + triplet.predicate.starts_with('?') as usize
        + triplet.object.starts_with('?') as usize;
    match placeholder_count {
        0 => PlaceholderTripletType::Resolved,
        1 => PlaceholderTripletType::Traceable,
        _ => PlaceholderTripletType::Fuzzy,
    }
}

fn has_structured_graph_input(req: &ExecutePlanRequest) -> bool {
    req.graph_hints.iter().any(|hint| {
        hint.subject
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || hint
                .predicate
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            || hint
                .object
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }) || req.placeholder_triplets.iter().any(|triplet| {
        !triplet.subject.trim().is_empty()
            || !triplet.predicate.trim().is_empty()
            || !triplet.object.trim().is_empty()
    })
}

/// Wire + budget validation for an execute-plan request (canonical implementation).
pub fn validate_execute_plan(req: &ExecutePlanRequest) -> Result<(), ExecutePlanValidationError> {
    if req.doc_scope.is_empty() {
        return Err(ExecutePlanValidationError::EmptyDocScope);
    }
    if req.items.is_empty() {
        return Err(ExecutePlanValidationError::EmptyItems);
    }
    if req.items.len() > MAX_EXECUTE_PLAN_ITEMS {
        return Err(ExecutePlanValidationError::TooManyItems {
            max: MAX_EXECUTE_PLAN_ITEMS,
        });
    }

    for (index, item) in req.items.iter().enumerate() {
        if !(0.0..=1.0).contains(&item.priority) {
            return Err(ExecutePlanValidationError::InvalidPriority { index });
        }

        let has_query = item
            .query
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_bm25_terms = item
            .bm25_terms
            .as_ref()
            .is_some_and(|terms| terms.iter().any(|term| !term.trim().is_empty()));
        if usize::from(has_query) + usize::from(has_bm25_terms) != 1 {
            return Err(ExecutePlanValidationError::InvalidPayloadCount { index });
        }
    }

    if req
        .budget
        .as_ref()
        .and_then(|budget| budget.total_candidate_budget)
        .is_some_and(|value| value == 0)
    {
        return Err(ExecutePlanValidationError::InvalidTotalCandidateBudget);
    }

    if req
        .budget
        .as_ref()
        .and_then(|budget| budget.final_chunk_budget)
        .is_some_and(|value| value == 0)
    {
        return Err(ExecutePlanValidationError::InvalidFinalChunkBudget);
    }

    for (index, triplet) in req.placeholder_triplets.iter().enumerate() {
        if triplet.placeholder_positions().len() > 2 {
            return Err(ExecutePlanValidationError::TooManyPlaceholders { index });
        }
    }

    if req
        .channel_budget
        .as_ref()
        .and_then(|budget| budget.graph)
        .is_some_and(|value| value > 0)
        && !has_structured_graph_input(req)
    {
        return Err(ExecutePlanValidationError::GraphBudgetRequiresHints);
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::rag_execute::{ChannelBudget, ExecutePlanItem, QueryEntity};

    fn base_request() -> ExecutePlanRequest {
        ExecutePlanRequest {
            plan_version: "v1".into(),
            doc_scope: vec!["doc-1".into()],
            items: vec![ExecutePlanItem {
                priority: 1.0,
                query: Some("hello".into()),
                bm25_terms: None,
            }],
            summary_mode: ExecutePlanSummaryMode::None,
            budget: None,
            channel_budget: None,
            query_entities: vec![],
            graph_hints: vec![],
            placeholder_triplets: vec![],
            trace: None,
        }
    }

    #[test]
    fn validate_rejects_empty_doc_scope() {
        let mut req = base_request();
        req.doc_scope.clear();
        assert!(matches!(
            validate_execute_plan(&req),
            Err(ExecutePlanValidationError::EmptyDocScope)
        ));
    }

    #[test]
    fn validate_rejects_ambiguous_items() {
        let mut req = base_request();
        req.items[0].bm25_terms = Some(vec!["also".into()]);
        assert!(matches!(
            validate_execute_plan(&req),
            Err(ExecutePlanValidationError::InvalidPayloadCount { index: 0 })
        ));
    }

    #[test]
    fn validate_rejects_too_many_items() {
        let mut req = base_request();
        req.items = (0..5)
            .map(|i| ExecutePlanItem {
                priority: 0.5,
                query: Some(format!("q{i}")),
                bm25_terms: None,
            })
            .collect();
        assert!(matches!(
            validate_execute_plan(&req),
            Err(ExecutePlanValidationError::TooManyItems { max: 4 })
        ));
    }

    #[test]
    fn validate_rejects_three_placeholders() {
        let mut req = base_request();
        req.placeholder_triplets = vec![PlaceholderTriplet {
            subject: "?s".into(),
            predicate: "?p".into(),
            object: "?o".into(),
        }];
        assert!(matches!(
            validate_execute_plan(&req),
            Err(ExecutePlanValidationError::TooManyPlaceholders { index: 0 })
        ));
    }

    #[test]
    fn validate_accepts_two_placeholders_with_graph_budget() {
        let mut req = base_request();
        req.channel_budget = Some(ChannelBudget {
            text_dense: None,
            bm25: None,
            multimodal_dense: None,
            graph: Some(4),
        });
        req.placeholder_triplets = vec![PlaceholderTriplet {
            subject: "Atlas".into(),
            predicate: "?p".into(),
            object: "?o".into(),
        }];
        assert!(validate_execute_plan(&req).is_ok());
    }

    #[test]
    fn validate_rejects_graph_budget_without_structure() {
        let mut req = base_request();
        req.channel_budget = Some(ChannelBudget {
            text_dense: None,
            bm25: None,
            multimodal_dense: None,
            graph: Some(4),
        });
        req.query_entities = vec![QueryEntity {
            text: "Atlas".into(),
            kind: Some("project".into()),
        }];
        assert!(matches!(
            validate_execute_plan(&req),
            Err(ExecutePlanValidationError::GraphBudgetRequiresHints)
        ));
    }

    #[test]
    fn injects_original_query_as_first_dense_item() {
        let mut req = base_request();
        req.items = vec![ExecutePlanItem {
            priority: 0.5,
            query: None,
            bm25_terms: Some(vec!["exact".into(), "term".into()]),
        }];
        ensure_original_query_text_dense_item(&mut req, "original question");
        assert_eq!(req.items[0].query.as_deref(), Some("original question"));
        assert_eq!(
            req.items[1].bm25_terms.as_ref().unwrap(),
            &vec!["exact".to_string(), "term".to_string()]
        );
        assert!(validate_execute_plan(&req).is_ok());
    }

    #[test]
    fn classify_resolved_triplet() {
        let t = PlaceholderTriplet {
            subject: "a".into(),
            predicate: "b".into(),
            object: "c".into(),
        };
        assert_eq!(
            classify_placeholder_triplet(&t),
            PlaceholderTripletType::Resolved
        );
    }

    #[test]
    fn chat_request_compat_maps_query() {
        let req = base_request();
        let chat = execute_plan_to_chat_request(&req);
        assert_eq!(chat.query, "hello");
        assert_eq!(chat.agent_type, "rag");
        assert_eq!(chat.doc_scope, vec!["doc-1".to_string()]);
    }
}
