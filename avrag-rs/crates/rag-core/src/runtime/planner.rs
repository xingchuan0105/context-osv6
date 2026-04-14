use anyhow::Result;
use avrag_llm::LlmUsage;
use common::{ChatRequest, DegradeTraceItem, RagPlan, RagPlanItem, RagTraceItem};
use uuid::Uuid;

use crate::context::SessionContext;

use super::RagRuntime;
use super::FINAL_RERANK_BUDGET;
use super::TOTAL_CANDIDATE_BUDGET;

fn default_rag_plan(query: &str) -> RagPlan {
    let mut items = vec![RagPlanItem {
        priority: 0.8,
        query: Some(query.to_string()),
        bm25_terms: None,
        summary: None,
    }];
    if let Some(filename_query) = extract_filename_query_hint(query) {
        let terms = filename_query
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !terms.is_empty() {
            items.push(RagPlanItem {
                priority: 0.2,
                query: None,
                bm25_terms: Some(terms),
                summary: None,
            });
        }
    }

    RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.5,
        clarify_needed: false,
        clarify_message: String::new(),
        items,
    }
}

fn extract_filename_query_hint(query: &str) -> Option<String> {
    query
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|ch: char| {
                ch.is_ascii_punctuation() && ch != '.' && ch != '-' && ch != '_'
            })
        })
        .find(|token| token.contains('.') && token.chars().any(|ch| ch.is_ascii_alphanumeric()))
        .map(|token| {
            token
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|value| !value.is_empty())
}

fn normalize_summary_payload(summary: Option<String>) -> Option<String> {
    match summary.as_deref().map(str::trim) {
        Some("all") => Some("all".to_string()),
        Some("related") => Some("related".to_string()),
        _ => None,
    }
}

pub(super) fn normalize_rag_plan(plan: &mut RagPlan, query: &str) {
    if plan.items.is_empty() && !plan.clarify_needed {
        plan.items = default_rag_plan(query).items;
    }

    for item in &mut plan.items {
        item.summary = normalize_summary_payload(item.summary.take());

        let has_query = item
            .query
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_bm25 = item
            .bm25_terms
            .as_ref()
            .is_some_and(|terms| !terms.is_empty());
        let has_summary = item.summary.is_some();
        let payload_count =
            usize::from(has_query) + usize::from(has_bm25) + usize::from(has_summary);

        if payload_count == 0 {
            item.query = Some(query.to_string());
        } else if payload_count > 1 {
            if has_query {
                item.bm25_terms = None;
                item.summary = None;
            } else if has_bm25 {
                item.summary = None;
            }
        }

        item.priority = item.priority.clamp(0.0, 1.0);
    }
}

pub(super) fn item_payload_kind(item: &RagPlanItem) -> &'static str {
    if item
        .summary
        .as_deref()
        .is_some_and(|value| matches!(value, "all" | "related"))
    {
        "summary"
    } else if item
        .bm25_terms
        .as_ref()
        .is_some_and(|terms| !terms.is_empty())
    {
        "bm25_terms"
    } else {
        "query"
    }
}

pub(super) fn effective_item_query(item: &RagPlanItem, default_query: &str) -> String {
    if let Some(query) = item.query.as_deref().filter(|value| !value.trim().is_empty()) {
        return query.trim().to_string();
    }
    if let Some(terms) = item.bm25_terms.as_ref().filter(|terms| !terms.is_empty()) {
        return terms
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    default_query.to_string()
}

pub(super) fn rag_summary_mode(plan: &RagPlan) -> String {
    plan.items
        .iter()
        .find_map(|item| {
            item.summary
                .as_deref()
                .filter(|value| matches!(*value, "all" | "related"))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "none".to_string())
}

pub(super) fn allocate_item_candidate_budgets(items: &[RagPlanItem]) -> Vec<usize> {
    let active_indices = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (item_payload_kind(item) != "summary").then_some(index))
        .collect::<Vec<_>>();
    if active_indices.is_empty() {
        return vec![0; items.len()];
    }

    let mut budgets = vec![0; items.len()];
    if active_indices.len() >= TOTAL_CANDIDATE_BUDGET {
        for index in active_indices.into_iter().take(TOTAL_CANDIDATE_BUDGET) {
            budgets[index] = 1;
        }
        return budgets;
    }

    for &index in &active_indices {
        budgets[index] = 1;
    }

    let remaining = TOTAL_CANDIDATE_BUDGET - active_indices.len();
    if remaining == 0 {
        return budgets;
    }

    let mut weights = active_indices
        .iter()
        .map(|&index| items[index].priority.clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    let total_weight: f32 = weights.iter().sum();
    if total_weight <= f32::EPSILON {
        weights.fill(1.0);
    }
    let normalized_total: f32 = weights.iter().sum();

    let mut remainders = Vec::with_capacity(active_indices.len());
    let mut assigned_extra = 0usize;
    for (&index, weight) in active_indices.iter().zip(weights.iter()) {
        let exact = if normalized_total <= f32::EPSILON {
            0.0
        } else {
            (*weight / normalized_total) * remaining as f32
        };
        let extra = exact.floor() as usize;
        budgets[index] += extra;
        assigned_extra += extra;
        remainders.push((index, exact - extra as f32));
    }

    remainders.sort_by(
        |(left_index, left_fraction), (right_index, right_fraction)| {
            right_fraction
                .partial_cmp(left_fraction)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left_index.cmp(right_index))
        },
    );

    let leftover = remaining.saturating_sub(assigned_extra);
    for (index, _) in remainders.into_iter().take(leftover) {
        budgets[index] += 1;
    }

    budgets
}

pub(super) fn request_doc_ids(request: &ChatRequest) -> Option<Vec<Uuid>> {
    (!request.doc_scope.is_empty()).then(|| {
        request
            .doc_scope
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect::<Vec<_>>()
    })
}

pub(super) fn build_item_trace(request: &ChatRequest, rag_plan: &RagPlan) -> Vec<RagTraceItem> {
    let candidate_budgets = allocate_item_candidate_budgets(&rag_plan.items);

    rag_plan
        .items
        .iter()
        .zip(candidate_budgets)
        .map(|(item, recall_budget)| {
            let payload_kind = item_payload_kind(item).to_string();
            let effective_query = effective_item_query(item, &request.query);
            RagTraceItem {
                priority: item.priority,
                payload_kind: payload_kind.clone(),
                query: item
                    .query
                    .clone()
                    .or_else(|| (payload_kind == "query").then_some(effective_query)),
                bm25_terms: item.bm25_terms.clone().unwrap_or_default(),
                summary: item.summary.clone(),
                recall_budget,
                bm25_k: if payload_kind == "bm25_terms" {
                    recall_budget
                } else {
                    0
                },
                dense_k: if payload_kind == "query" {
                    recall_budget
                } else {
                    0
                },
                rerank_budget: FINAL_RERANK_BUDGET,
                source_count: 0,
                source_ids: Vec::new(),
            }
        })
        .collect()
}

pub(super) fn planner_session_context(session_context: Option<&SessionContext>) -> Option<String> {
    let Some(session_context) = session_context else {
        return None;
    };

    let mut parts = Vec::new();
    if let Some(summary) = session_context
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
    {
        parts.push(format!("Conversation summary:\n{summary}"));
    }

    let recent_messages = session_context
        .messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| format!("{}: {}", message.role, message.content.trim()))
        .collect::<Vec<_>>();
    if !recent_messages.is_empty() {
        parts.push(format!("Recent messages:\n{}", recent_messages.join("\n")));
    }

    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

impl RagRuntime {
    pub async fn plan(
        &self,
        request: &ChatRequest,
        session_context: Option<&SessionContext>,
        docscope_metadata: Option<&common::DocScopeMetadata>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> Result<(RagPlan, Option<LlmUsage>)> {
        let query = request.query.as_str();
        let planner_context = planner_session_context(session_context);
        let mut rag_plan = default_rag_plan(query);
        let mut planner_usage: Option<LlmUsage> = None;

        if let Some(planner) = &self.config.planner {
            match planner
                .plan_with_usage(query, planner_context.as_deref(), docscope_metadata)
                .await
            {
                Ok((plan, usage)) => {
                    rag_plan = plan;
                    planner_usage = Some(usage);
                }
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "planner".to_string(),
                        reason: format!("Planner call failed: {}", error),
                        impact: "Using default retrieval".to_string(),
                    });
                }
            }
        }

        normalize_rag_plan(&mut rag_plan, query);
        Ok((rag_plan, planner_usage))
    }

    pub fn normalize_plan(
        &self,
        request: &ChatRequest,
        rag_plan: &mut RagPlan,
    ) -> Vec<RagTraceItem> {
        normalize_rag_plan(rag_plan, &request.query);
        build_item_trace(request, rag_plan)
    }
}
