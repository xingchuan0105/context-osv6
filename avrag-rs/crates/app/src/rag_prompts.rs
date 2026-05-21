use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use avrag_llm::LlmUsage;
use common::{
    AnswerContextChunk, ChatRequest, ExecutePlanItem, ExecutePlanRequest, ExecutePlanResponse,
    ExecutePlanSummaryMode, GraphHint, PlaceholderTriplet, QueryEntity, RetrievalPlannerOutput,
    ToolCall,
};

const RAG_EXECUTE_PLAN_VERSION: &str = "rag-execute-v1";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RagStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    pub recommendation: StrategyRecommendation,
    pub reason: String,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyDimension {
    pub name: String,
    #[serde(default)]
    pub attempted: bool,
    #[serde(default)]
    pub covered: bool,
    #[serde(default)]
    pub retrieved_count: usize,
    #[serde(default)]
    pub query_ids: Vec<String>,
    pub status: DimensionStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyRecommendation {
    Synthesize,
    Replan,
    Broaden,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionStatus {
    CoveredStrong,
    CoveredWeak,
    Missing,
}

// ---------------- Search strategy evaluation ----------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    pub recommendation: SearchStrategyRecommendation,
    pub reason: String,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStrategyRecommendation {
    Synthesize,
    Broaden,
    EscalateVertical,
}

/// Per-sub-query item used to build the strategy evaluation prompt.
/// `tool_index` maps this sub-query back to the `tool_results` array so
/// result counts are reported against the correct tool call.
#[derive(Debug, Clone)]
pub(crate) struct SubQueryItem {
    pub id: String,
    pub text: String,
    pub tool_index: usize,
}

/// Plan strategy emitted by the PLAN phase LLM (P4 format).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStrategy {
    pub strategy: Vec<PlanStrategyItem>,
    #[serde(default = "default_next_step_str")]
    pub next_step: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStrategyItem {
    pub tool: String,
    #[serde(flatten)]
    pub params: serde_json::Value,
}

fn default_next_step_str() -> String {
    "answer".to_string()
}

#[derive(Debug, Clone)]
pub enum RagPlanDecision {
    ToolCalls(Vec<ToolCall>),
    Strategy(PlanStrategy),
    Clarify(String),
}

#[derive(Debug, Clone)]
pub struct RagPlanResult {
    pub decision: RagPlanDecision,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct RagAnswerResult {
    pub answer_text: String,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct RagBehaviorSkill {
    pub name: String,
    pub instructions: Vec<String>,
}

impl RagBehaviorSkill {
    fn new(
        name: impl Into<String>,
        instructions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            instructions: instructions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RagContext {
    pub mode: String,
    pub current_task: String,
    pub authoritative_context: String,
    pub reference_context: String,
    pub user_preference_memory: String,
    pub skill: RagBehaviorSkill,
    pub output_contract: String,
}

pub fn answer_context(response: &ExecutePlanResponse) -> Vec<AnswerContextChunk> {
    response.bundle.answer_context_chunks()
}

#[allow(dead_code)]
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

    // Inject previously retrieved doc_index results so the planner can issue index_lookup.
    let doc_index_results: Vec<&serde_json::Value> = previous_tool_results
        .iter()
        .filter(|r| r.tool == "doc_index" && r.status == common::ToolStatus::Ok)
        .filter_map(|r| r.data.as_ref())
        .collect();
    if !doc_index_results.is_empty() {
        let index_json = serde_json::to_string_pretty(&doc_index_results)
            .unwrap_or_else(|_| "[]".to_string());
        authoritative.push_str(&format!(
            "\n\nDocument index already retrieved (from prior iteration):\n{}\n\n\
             Based on this index, call index_lookup for the sections needed to answer the user.",
            index_json
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
                "Ask one natural-language clarification question when retrieval cannot proceed.",
            ],
        ),
        output_contract: "Return exactly one raw JSON object: either PlanStrategy ({\"strategy\":[{tool, param1, param2}],\"next_step\":\"answer\"}) or {\"action\":\"clarify\",\"message\":\"...\"}.".to_string(),
    })
}

pub(crate) fn parse_rag_plan_decision(raw: &str, request: &ChatRequest) -> Option<(RagPlanDecision, Vec<String>)> {
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
            return Some((RagPlanDecision::Clarify(message.to_string()), Vec::new()));
        }

    // 2. P4 format: PlanStrategy (plan-only, no schema-compliant args yet)
    if let Ok(strategy) = serde_json::from_str::<PlanStrategy>(&json)
        && !strategy.strategy.is_empty() {
            return Some((RagPlanDecision::Strategy(strategy), Vec::new()));
        }

    // 3. Phase-3c format: RetrievalPlannerOutput (ToolCall[])
    if let Ok(planner_output) = serde_json::from_str::<RetrievalPlannerOutput>(&json)
        && !planner_output.calls.is_empty() {
            // Phase-3c: bypass adapter — return raw ToolCalls for the dispatcher
            return Some((RagPlanDecision::ToolCalls(planner_output.calls), planner_output.skills));
        }

    // 4. Legacy format: ExecutePlanRequest (backward compatibility)
    let plan = serde_json::from_str::<ExecutePlanRequest>(&json).ok()?;
    if plan.validate().is_err() || plan.doc_scope != request.doc_scope {
        return None;
    }
    match normalize_execute_plan_request(plan, request) {
        Some(plan) => Some((RagPlanDecision::ToolCalls(execute_plan_request_to_tool_calls(plan)), Vec::new())),
        None => Some((RagPlanDecision::Clarify(
            crate::chat::i18n::clarify::need_query_or_doc_scope(request.language.as_deref())
                .to_string(),
        ), Vec::new())),
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

pub fn extract_referenced_chunk_ids(answer_text: &str) -> HashSet<String> {
    let mut remaining = answer_text;
    let mut ids = HashSet::new();
    while let Some(start) = remaining.find("[[") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if !chunk_id.is_empty() {
                ids.insert(chunk_id.to_string());
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim)
            && !chunk_id.is_empty()
        {
            ids.insert(chunk_id.to_string());
        }
        remaining = &after_start[end + 2..];
    }
    ids
}

fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
}

fn build_rag_envelope(context: RagContext) -> String {
    format!(
        "<Mode>\n{}\n\n<Current Task>\n{}\n\n<Authoritative Context>\n{}\n\n<Reference Context>\n{}\n\n<User Preference Memory>\n{}\n\n<Behavior Skill>\n{}\n\n<Output Contract>\n{}",
        context.mode,
        context.current_task,
        context.authoritative_context,
        context.reference_context,
        context.user_preference_memory,
        format_behavior_skill(&context.skill),
        context.output_contract,
    )
}

fn format_behavior_skill(skill: &RagBehaviorSkill) -> String {
    let instructions = if skill.instructions.is_empty() {
        "- none".to_string()
    } else {
        skill
            .instructions
            .iter()
            .map(|instruction| format!("- {instruction}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("name: {}\ninstructions:\n{}", skill.name, instructions)
}

fn normalize_execute_plan_item(item: ExecutePlanItem) -> Option<ExecutePlanItem> {
    let query = item
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let bm25_terms = item.bm25_terms.map(|terms| {
        terms
            .into_iter()
            .map(|term| term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>()
    });
    let has_query = query.is_some();
    let has_bm25_terms = bm25_terms.as_ref().is_some_and(|terms| !terms.is_empty());

    if has_query {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query,
            bm25_terms: None,
        })
    } else if has_bm25_terms {
        Some(ExecutePlanItem {
            priority: item.priority.clamp(0.0, 1.0),
            query: None,
            bm25_terms,
        })
    } else {
        None
    }
}

fn normalize_query_entities(entities: Vec<QueryEntity>) -> Vec<QueryEntity> {
    let mut seen = HashSet::new();
    entities
        .into_iter()
        .filter_map(|entity| {
            let text = entity.text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let key = text.to_lowercase();
            if !seen.insert(key) {
                return None;
            }
            Some(QueryEntity {
                text,
                kind: entity
                    .kind
                    .as_deref()
                    .map(str::trim)
                    .filter(|kind| !kind.is_empty())
                    .map(ToOwned::to_owned),
            })
        })
        .collect()
}

fn normalize_graph_hints(hints: Vec<GraphHint>) -> Vec<GraphHint> {
    hints
        .into_iter()
        .filter_map(|hint| {
            let subject = hint
                .subject
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let predicate = hint
                .predicate
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let object = hint
                .object
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            (subject.is_some() || predicate.is_some() || object.is_some()).then_some(GraphHint {
                subject,
                predicate,
                object,
            })
        })
        .collect()
}

fn normalize_placeholder_triplets(triplets: Vec<PlaceholderTriplet>) -> Vec<PlaceholderTriplet> {
    let mut seen = HashSet::new();
    triplets
        .into_iter()
        .filter_map(|triplet| {
            let subject = triplet.subject.trim().to_string();
            let predicate = triplet.predicate.trim().to_string();
            let object = triplet.object.trim().to_string();
            if subject.is_empty() || predicate.is_empty() || object.is_empty() {
                return None;
            }
            let key = (
                subject.to_lowercase(),
                predicate.to_lowercase(),
                object.to_lowercase(),
            );
            seen.insert(key).then_some(PlaceholderTriplet {
                subject,
                predicate,
                object,
            })
        })
        .take(6)
        .collect()
}

#[allow(dead_code)]
fn build_chunk_id_reference_table(
    answer_context: &[AnswerContextChunk],
    citations: &[common::Citation],
) -> String {
    if answer_context.is_empty() {
        return "No chunks available.".to_string();
    }

    let citation_by_chunk: std::collections::HashMap<String, &common::Citation> = citations
        .iter()
        .filter_map(|c| c.chunk_id.as_ref().map(|id| (id.clone(), c)))
        .collect();

    let mut lines = vec!["Available chunk IDs for citation:".to_string()];
    for chunk in answer_context.iter().take(20) {
        let doc_name = citation_by_chunk
            .get(&chunk.chunk_id)
            .map(|c| c.doc_name.as_str())
            .unwrap_or("unknown");
        let preview = chunk.text.chars().take(80).collect::<String>();
        lines.push(format!(
            "  - CHUNK_ID: {} | Doc: {} | Preview: {}...",
            chunk.chunk_id, doc_name, preview
        ));
    }
    if answer_context.len() > 20 {
        lines.push(format!("  ... and {} more chunks", answer_context.len() - 20));
    }
    lines.push("".to_string());
    lines.push("Citation syntax:".to_string());
    lines.push("  [[cite:CHUNK_ID]] - reference a text chunk".to_string());
    lines.push("  [[image:CHUNK_ID]] - reference an image chunk".to_string());
    lines.join("\n")
}

pub(crate) fn build_rag_strategy_evaluation_prompt(
    query: &str,
    sub_queries: &[SubQueryItem],
    tool_results: &[common::ToolResult],
    accumulated_chunk_count: usize,
    iteration: u8,
) -> String {
    let sub_query_lines: Vec<String> = sub_queries
        .iter()
        .map(|item| {
            let count = tool_results
                .get(item.tool_index)
                .and_then(|r| r.data.as_ref().and_then(|d| d.as_array()).map(|a| a.len()))
                .unwrap_or(0);
            let status = tool_results.get(item.tool_index).map_or("unknown".to_string(), |r| {
                if r.status == common::ToolStatus::Ok {
                    format!("{} results", count)
                } else {
                    format!("{:?}", r.status)
                }
            });
            format!("- {}: \"{}\" -> {}", item.id, item.text, status)
        })
        .collect();

    let mapped_indices: std::collections::HashSet<usize> =
        sub_queries.iter().map(|item| item.tool_index).collect();

    let extra_tools: Vec<String> = tool_results
        .iter()
        .enumerate()
        .filter(|(idx, _)| !mapped_indices.contains(idx))
        .map(|(_, r)| {
            let count = r
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if r.status == common::ToolStatus::Ok {
                format!("- tool={} -> {} results", r.tool, count)
            } else {
                format!("- tool={} -> {:?}", r.tool, r.status)
            }
        })
        .collect();

    let tools_line = if !extra_tools.is_empty() {
        format!("\nAdditional tool calls:\n{}", extra_tools.join("\n"))
    } else {
        String::new()
    };

    let doc_index_hint = {
        let has_doc_index = tool_results
            .iter()
            .any(|r| r.tool == "doc_index" && r.status == common::ToolStatus::Ok);
        let has_index_lookup = tool_results
            .iter()
            .any(|r| r.tool == "index_lookup" && r.status == common::ToolStatus::Ok);
        if has_doc_index && !has_index_lookup {
            "\n\nNote: Document index was retrieved but section content (index_lookup) has not been fetched yet. If the user's question requires reading specific sections, recommend Replan and suggest calling index_lookup with the relevant chunk_ids from the document index."
        } else {
            ""
        }
    };

    format!(
        "User's original question:\n{}\n\n\
         Executed sub-queries (iteration {}):\n{}{}\n\n\
         Accumulated across all iterations so far:\n  - unique chunks: {}\n\n\
         Evaluate retrieval coverage.{}",
        query.trim(),
        iteration + 1,
        sub_query_lines.join("\n"),
        tools_line,
        accumulated_chunk_count,
        doc_index_hint,
    )
}

pub(crate) fn parse_rag_strategy_evaluation(raw: &str) -> Option<RagStrategyEvaluation> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    serde_json::from_str::<RagStrategyEvaluation>(&json).ok()
}

pub(crate) fn build_search_strategy_evaluation_prompt(
    query: &str,
    vertical: Option<&str>,
    sub_queries: &[String],
    result_count: usize,
    accumulated_count: usize,
    iteration: u8,
) -> String {
    let sub_query_lines: Vec<String> = sub_queries
        .iter()
        .enumerate()
        .map(|(i, sq)| format!("- q{}: \"{}\"", i + 1, sq))
        .collect();

    let vertical_line = vertical
        .map(|v| format!("\nVertical used: {}", v))
        .unwrap_or_default();

    format!(
        "User's original question:\n{}\n\n\
         Executed search queries (iteration {}):\n{}{}\n\n\
         Results from this iteration:\n  - total results: {}\n\n\
         Accumulated across all iterations so far:\n  - unique sources: {}\n\n\
         Evaluate search coverage.",
        query.trim(),
        iteration + 1,
        sub_query_lines.join("\n"),
        vertical_line,
        result_count,
        accumulated_count,
    )
}

pub(crate) fn parse_search_strategy_evaluation(raw: &str) -> Option<SearchStrategyEvaluation> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    serde_json::from_str::<SearchStrategyEvaluation>(&json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(agent_type: &str, query: &str, doc_scope: &[&str]) -> ChatRequest {
        ChatRequest {
            query: query.to_string(),
            notebook_id: None,
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope: doc_scope.iter().map(|value| value.to_string()).collect(),
            messages: Vec::new(),
            stream: false,
            language: None,
        }
    }

    fn sample_execute_response() -> ExecutePlanResponse {
        ExecutePlanResponse {
            bundle: common::RetrievalBundle {
                chunks: vec![common::RetrievedChunk {
                    chunk_id: "chunk-1".to_string(),
                    doc_id: "doc-1".to_string(),
                    chunk_type: "text".to_string(),
                    page: Some(1),
                    text: "retrieved".to_string(),
                    score: 0.9,
                    retrieval_channel: "dense".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                    score_breakdown: Vec::new(),
                }],
                graph_supported_chunks: Vec::new(),
                relation_paths: Vec::new(),
                citations: vec![common::Citation {
                    citation_id: 1,
                    doc_id: "doc-1".to_string(),
                    chunk_id: Some("chunk-1".to_string()),
                    page: Some(1),
                    doc_name: "Document 1".to_string(),
                    preview: Some("retrieved".to_string()),
                    content: Some("retrieved".to_string()),
                    score: 0.9,
                    layer: Some("dense".to_string()),
                    chunk_type: Some("text".to_string()),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                }],
                summary_chunks: Vec::new(),
            },
            coverage: common::Coverage {
                requested_doc_count: 1,
                matched_doc_count: 1,
                retrieved_chunk_count: 1,
                summary_chunk_count: 0,
                channel_coverage: Default::default(),
            },
            degrade_trace: Vec::new(),
            backend_trace: common::BackendTrace {
                trace: None,
                item_trace: vec![common::RagTraceItem {
                    priority: 1.0,
                    payload_kind: "query".to_string(),
                    query: Some("test".to_string()),
                    bm25_terms: Vec::new(),
                    summary: None,
                    recall_budget: 100,
                    bm25_k: 0,
                    dense_k: 100,
                    rerank_budget: 100,
                    source_count: 1,
                    source_ids: vec!["chunk-1".to_string()],
                }],
                channel_trace: Vec::new(),
                retrieval_trace: common::RagTraceSummary {
                    item_count: 1,
                    total_candidate_budget: 100,
                    max_rerank_docs: 100,
                    max_final_chunks: 30,
                    top_k_returned: 1,
                    summary_mode: "none".to_string(),
                    items: Vec::new(),
                },
            },
        }
    }

    #[test]
    fn rag_envelope_formats_behavior_skill_profile_without_tools() {
        let envelope = build_rag_envelope(RagContext {
            mode: "rag-answer".to_string(),
            current_task: "summarize".to_string(),
            authoritative_context: "evidence".to_string(),
            reference_context: "none".to_string(),
            user_preference_memory: "none".to_string(),
            skill: RagBehaviorSkill {
                name: "rag-answer".to_string(),
                instructions: vec![
                    "Use only RAG Evidence for factual claims.".to_string(),
                    "Use preferences only for expression style.".to_string(),
                ],
            },
            output_contract: "Return natural language.".to_string(),
        });

        assert!(envelope.contains("<Behavior Skill>"));
        assert!(envelope.contains("name: rag-answer"));
        assert!(envelope.contains("- Use only RAG Evidence for factual claims."));
        assert!(!envelope.contains("<Tools>"));
        assert!(!envelope.contains("tool_schema"));
    }

    #[test]
    fn execute_plan_bundle_consumption_preserves_retrieval_then_summary_order() {
        let response = ExecutePlanResponse {
            bundle: common::RetrievalBundle {
                chunks: vec![common::RetrievedChunk {
                    chunk_id: "chunk-1".to_string(),
                    doc_id: "doc-1".to_string(),
                    chunk_type: "text".to_string(),
                    page: Some(1),
                    text: "retrieved".to_string(),
                    score: 0.9,
                    retrieval_channel: "dense".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                    score_breakdown: Vec::new(),
                }],
                graph_supported_chunks: vec![common::RetrievedChunk {
                    chunk_id: "graph-chunk-1".to_string(),
                    doc_id: "doc-1".to_string(),
                    chunk_type: "text".to_string(),
                    page: Some(2),
                    text: "graph supported".to_string(),
                    score: 0.8,
                    retrieval_channel: "graph".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                    score_breakdown: Vec::new(),
                }],
                relation_paths: Vec::new(),
                citations: Vec::new(),
                summary_chunks: vec![common::AnswerContextChunk {
                    chunk_id: "summary-doc-1".to_string(),
                    doc_id: Some("doc-1".to_string()),
                    chunk_type: "summary".to_string(),
                    page: None,
                    text: "[Document Summary] summary".to_string(),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                }],
            },
            coverage: common::Coverage {
                requested_doc_count: 1,
                matched_doc_count: 1,
                retrieved_chunk_count: 1,
                summary_chunk_count: 1,
                channel_coverage: Default::default(),
            },
            degrade_trace: Vec::new(),
            backend_trace: common::BackendTrace {
                trace: None,
                item_trace: Vec::new(),
                channel_trace: Vec::new(),
                retrieval_trace: common::RagTraceSummary {
                    item_count: 0,
                    total_candidate_budget: 0,
                    max_rerank_docs: 0,
                    max_final_chunks: 0,
                    top_k_returned: 1,
                    summary_mode: "related".to_string(),
                    items: Vec::new(),
                },
            },
        };

        let answer_context = answer_context(&response);
        assert_eq!(answer_context.len(), 3);
        assert_eq!(answer_context[0].chunk_type, "text");
        assert_eq!(answer_context[1].chunk_id, "graph-chunk-1");
        assert_eq!(answer_context[2].chunk_type, "summary");
    }

    #[test]
    fn normalize_execute_plan_request_preserves_graph_hints() {
        let request = request("rag", "how does Atlas use the checklist?", &["doc-1"]);
        let plan = ExecutePlanRequest {
            plan_version: "rag-execute-v1".to_string(),
            doc_scope: vec!["ignored-doc".to_string()],
            items: vec![ExecutePlanItem {
                priority: 1.0,
                query: Some("Atlas checklist".to_string()),
                bm25_terms: None,
            }],
            summary_mode: ExecutePlanSummaryMode::None,
            budget: None,
            channel_budget: None,
            query_entities: vec![
                QueryEntity {
                    text: " Atlas ".to_string(),
                    kind: Some(" project ".to_string()),
                },
                QueryEntity {
                    text: "atlas".to_string(),
                    kind: None,
                },
            ],
            graph_hints: vec![GraphHint {
                subject: Some(" Atlas ".to_string()),
                predicate: Some(" uses ".to_string()),
                object: Some(" rollback checklist ".to_string()),
            }],
            placeholder_triplets: vec![
                PlaceholderTriplet {
                    subject: " Atlas ".to_string(),
                    predicate: " uses ".to_string(),
                    object: " ?checklist ".to_string(),
                },
                PlaceholderTriplet {
                    subject: "atlas".to_string(),
                    predicate: "uses".to_string(),
                    object: "?checklist".to_string(),
                },
            ],
            trace: None,
        };

        let normalized = normalize_execute_plan_request(plan, &request).unwrap();

        assert_eq!(normalized.doc_scope, vec!["doc-1".to_string()]);
        assert_eq!(normalized.query_entities.len(), 1);
        assert_eq!(normalized.query_entities[0].text, "Atlas");
        assert_eq!(
            normalized.query_entities[0].kind.as_deref(),
            Some("project")
        );
        assert_eq!(normalized.graph_hints[0].predicate.as_deref(), Some("uses"));
        assert_eq!(normalized.placeholder_triplets.len(), 1);
        assert_eq!(normalized.placeholder_triplets[0].object, "?checklist");
    }

    #[test]
    fn parse_rag_plan_rejects_raw_invalid_payload_before_normalize() {
        let request = request("rag", "find rollback checklist", &["doc-1"]);
        let raw = serde_json::json!({
            "plan_version": "rag-execute-v1",
            "doc_scope": ["doc-1"],
            "items": [{
                "priority": 1.0,
                "query": "semantic lookup",
                "bm25_terms": ["exact"]
            }],
            "summary_mode": "none"
        })
        .to_string();

        assert!(parse_rag_plan_decision(&raw, &request).is_none());
    }

    #[test]
    fn parse_rag_plan_rejects_raw_doc_scope_mismatch_before_normalize() {
        let request = request("rag", "find rollback checklist", &["doc-1"]);
        let raw = serde_json::json!({
            "plan_version": "rag-execute-v1",
            "doc_scope": ["other-doc"],
            "items": [{ "priority": 1.0, "query": "semantic lookup" }],
            "summary_mode": "none"
        })
        .to_string();

        assert!(parse_rag_plan_decision(&raw, &request).is_none());
    }

    #[test]
    fn parse_rag_plan_accepts_new_tool_call_format() {
        let request = request("rag", "How does Atlas handle rollback?", &["doc-1"]);
        let raw = serde_json::json!({
            "calls": [
                { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["Atlas rollback mechanism"], "modality": "text", "top_k": 10 } }
            ],
            "next_step": "answer"
        })
        .to_string();

        let decision = parse_rag_plan_decision(&raw, &request);
        assert!(
            matches!(decision, Some((RagPlanDecision::ToolCalls(ref calls), _)) if calls.len() == 1),
            "expected ToolCalls with 1 call, got {:?}",
            decision
        );
    }

    #[test]
    fn parse_rag_plan_accepts_legacy_execute_plan_request() {
        let request = request("rag", "find rollback checklist", &["doc-1"]);
        let raw = serde_json::json!({
            "plan_version": "rag-execute-v1",
            "doc_scope": ["doc-1"],
            "items": [{ "priority": 1.0, "query": "rollback checklist" }],
            "summary_mode": "none"
        })
        .to_string();

        let decision = parse_rag_plan_decision(&raw, &request);
        assert!(
            matches!(decision, Some((RagPlanDecision::ToolCalls(ref calls), _)) if calls.len() == 1 && calls[0].tool == "dense_retrieval"),
            "expected ToolCalls with 1 dense_retrieval call, got {:?}",
            decision
        );
    }

    #[test]
    fn parse_rag_plan_accepts_any_tool_in_new_format() {
        let request = request("rag", "read chapter 3", &["doc-1"]);
        let raw = serde_json::json!({
            "calls": [
                { "tool": "index_lookup", "version": "1.0", "args": { "doc_id": "doc-1", "chunk_ids": ["c1"] } }
            ],
            "next_step": "answer"
        })
        .to_string();

        // Phase-3c: adapter is bypassed — any valid ToolCall is accepted raw
        let decision = parse_rag_plan_decision(&raw, &request);
        assert!(
            matches!(decision, Some((RagPlanDecision::ToolCalls(ref calls), _)) if calls.len() == 1),
            "expected ToolCalls with 1 call, got {:?}",
            decision
        );
    }

    #[test]
    fn parse_rag_plan_accepts_p4_plan_strategy_format() {
        let request = request("rag", "How does Atlas handle rollback?", &["doc-1"]);
        let raw = serde_json::json!({
            "strategy": [
                { "tool": "dense_retrieval", "queries": ["Atlas rollback mechanism"] },
                { "tool": "lexical_retrieval", "terms": ["FE-2048", "PRD"] }
            ],
            "next_step": "answer"
        })
        .to_string();

        let decision = parse_rag_plan_decision(&raw, &request);
        assert!(
            matches!(decision, Some((RagPlanDecision::Strategy(ref s), _)) if s.strategy.len() == 2),
            "expected Strategy with 2 items, got {:?}",
            decision
        );
        if let Some((RagPlanDecision::Strategy(s), _)) = decision {
            assert_eq!(s.strategy[0].tool, "dense_retrieval");
            assert_eq!(s.strategy[1].tool, "lexical_retrieval");
        }
    }

    #[test]
    fn plan_strategy_to_tool_calls_converts_items_directly() {
        let strategy = PlanStrategy {
            strategy: vec![
                PlanStrategyItem {
                    tool: "dense_retrieval".to_string(),
                    params: serde_json::json!({ "queries": ["q1"], "modality": "text", "top_k": 10 }),
                },
                PlanStrategyItem {
                    tool: "lexical_retrieval".to_string(),
                    params: serde_json::json!({ "terms": ["a", "b"], "top_k": 5 }),
                },
            ],
            next_step: "answer".to_string(),
        };

        let calls = plan_strategy_to_tool_calls(&strategy);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "dense_retrieval");
        assert_eq!(calls[0].version, "1.0");
        assert_eq!(
            calls[0].args,
            serde_json::json!({ "queries": ["q1"], "modality": "text", "top_k": 10 })
        );
        assert_eq!(calls[1].tool, "lexical_retrieval");
    }

    #[test]
    fn plan_strategy_to_tool_calls_handles_empty_strategy() {
        let strategy = PlanStrategy {
            strategy: vec![],
            next_step: "answer".to_string(),
        };
        let calls = plan_strategy_to_tool_calls(&strategy);
        assert!(calls.is_empty());
    }

    // ---------------- strategy evaluation prompt / parser ----------------

    #[test]
    fn build_rag_strategy_evaluation_prompt_contains_all_inputs() {
        let tool_results = vec![
            common::ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: common::ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "text": "alpha"},
                    {"chunk_id": "c2", "text": "beta"},
                ])),
                trace: None,
            },
            common::ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: common::ToolStatus::Ok,
                data: Some(serde_json::json!([])),
                trace: None,
            },
            common::ToolResult {
                tool: "graph_retrieval".to_string(),
                version: "1.0".to_string(),
                status: common::ToolStatus::Error,
                data: None,
                trace: None,
            },
        ];

        let sub_queries = vec![
            SubQueryItem {
                id: "q1".to_string(),
                text: "rust async runtime".to_string(),
                tool_index: 0,
            },
            SubQueryItem {
                id: "q2".to_string(),
                text: "BM25: async, runtime, tokio".to_string(),
                tool_index: 1,
            },
        ];

        let prompt = build_rag_strategy_evaluation_prompt(
            "How does async runtime work in Rust?",
            &sub_queries,
            &tool_results,
            5,
            1,
        );

        assert!(prompt.contains("How does async runtime work in Rust?"));
        assert!(prompt.contains("iteration 2"));
        assert!(prompt.contains("- q1: \"rust async runtime\" -> 2 results"));
        assert!(prompt.contains("- q2: \"BM25: async, runtime, tokio\" -> 0 results"));
        assert!(prompt.contains("Additional tool calls:"));
        assert!(prompt.contains("tool=graph_retrieval -> Error"));
        assert!(prompt.contains("unique chunks: 5"));
    }

    #[test]
    fn build_rag_strategy_evaluation_prompt_maps_multi_query_tool_correctly() {
        // One dense_retrieval call with 2 queries → both map to tool_index 0
        let tool_results = vec![
            common::ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: common::ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "text": "alpha"},
                    {"chunk_id": "c2", "text": "beta"},
                    {"chunk_id": "c3", "text": "gamma"},
                ])),
                trace: None,
            },
        ];

        let sub_queries = vec![
            SubQueryItem {
                id: "q1".to_string(),
                text: "query A".to_string(),
                tool_index: 0,
            },
            SubQueryItem {
                id: "q2".to_string(),
                text: "query B".to_string(),
                tool_index: 0,
            },
        ];

        let prompt = build_rag_strategy_evaluation_prompt(
            "test",
            &sub_queries,
            &tool_results,
            0,
            0,
        );

        // Both q1 and q2 should report 3 results (from the same tool_result at index 0)
        assert!(prompt.contains("- q1: \"query A\" -> 3 results"));
        assert!(prompt.contains("- q2: \"query B\" -> 3 results"));
        assert!(!prompt.contains("Additional tool calls"));
    }

    #[test]
    fn parse_rag_strategy_evaluation_parses_valid_json() {
        let raw = r#"{"dimensions": [{"name": "async runtime", "attempted": true, "covered": true, "retrieved_count": 3, "query_ids": ["q1"], "status": "covered_strong"}], "missing_dimensions": ["memory model"], "weak_dimensions": [], "recommendation": "replan", "reason": "missing memory model dimension", "suggested_followup_queries": ["memory model async rust"] }"#;
        let eval = parse_rag_strategy_evaluation(raw).unwrap();
        assert_eq!(eval.dimensions.len(), 1);
        assert_eq!(eval.dimensions[0].name, "async runtime");
        assert!(eval.dimensions[0].attempted);
        assert!(eval.dimensions[0].covered);
        assert_eq!(eval.dimensions[0].retrieved_count, 3);
        assert_eq!(eval.dimensions[0].query_ids, vec!["q1"]);
        assert!(matches!(eval.dimensions[0].status, DimensionStatus::CoveredStrong));
        assert_eq!(eval.missing_dimensions, vec!["memory model"]);
        assert!(eval.weak_dimensions.is_empty());
        assert!(matches!(eval.recommendation, StrategyRecommendation::Replan));
        assert_eq!(eval.reason, "missing memory model dimension");
        assert_eq!(eval.suggested_followup_queries, vec!["memory model async rust"]);
    }

    #[test]
    fn parse_rag_strategy_evaluation_parses_snake_case_recommendations() {
        let synthesize = r#"{"recommendation": "synthesize", "reason": "done", "status": "covered_strong"}"#;
        let replan = r#"{"recommendation": "replan", "reason": "missing", "status": "missing"}"#;
        let broaden = r#"{"recommendation": "broaden", "reason": "too few", "status": "covered_weak"}"#;

        assert!(matches!(
            parse_rag_strategy_evaluation(synthesize).unwrap().recommendation,
            StrategyRecommendation::Synthesize
        ));
        assert!(matches!(
            parse_rag_strategy_evaluation(replan).unwrap().recommendation,
            StrategyRecommendation::Replan
        ));
        assert!(matches!(
            parse_rag_strategy_evaluation(broaden).unwrap().recommendation,
            StrategyRecommendation::Broaden
        ));
    }

    #[test]
    fn parse_rag_strategy_evaluation_parses_all_dimension_statuses() {
        let raw = r#"{"dimensions": [
            {"name": "a", "status": "covered_strong"},
            {"name": "b", "status": "covered_weak"},
            {"name": "c", "status": "missing"}
        ], "recommendation": "synthesize", "reason": "ok"}"#;
        let eval = parse_rag_strategy_evaluation(raw).unwrap();
        assert!(matches!(eval.dimensions[0].status, DimensionStatus::CoveredStrong));
        assert!(matches!(eval.dimensions[1].status, DimensionStatus::CoveredWeak));
        assert!(matches!(eval.dimensions[2].status, DimensionStatus::Missing));
    }

    #[test]
    fn parse_rag_strategy_evaluation_handles_json_wrapped_in_markdown() {
        let raw = r#"Here is my evaluation:
```json
{"dimensions": [], "missing_dimensions": [], "weak_dimensions": [], "recommendation": "synthesize", "reason": "complete", "suggested_followup_queries": []}
```"#;
        let eval = parse_rag_strategy_evaluation(raw).unwrap();
        assert!(matches!(eval.recommendation, StrategyRecommendation::Synthesize));
        assert_eq!(eval.reason, "complete");
    }

    #[test]
    fn parse_rag_strategy_evaluation_returns_none_for_invalid_json() {
        let raw = "this is not json at all";
        assert!(parse_rag_strategy_evaluation(raw).is_none());
    }

    #[test]
    fn parse_rag_strategy_evaluation_uses_defaults_for_optional_fields() {
        let raw = r#"{"recommendation": "synthesize", "reason": "ok"}"#;
        let eval = parse_rag_strategy_evaluation(raw).unwrap();
        assert!(eval.dimensions.is_empty());
        assert!(eval.missing_dimensions.is_empty());
        assert!(eval.weak_dimensions.is_empty());
        assert!(eval.suggested_followup_queries.is_empty());
    }

    // ---------------- search strategy evaluation prompt / parser ----------------

    #[test]
    fn build_search_strategy_evaluation_prompt_contains_all_inputs() {
        let prompt = build_search_strategy_evaluation_prompt(
            "What is the latest on Rust async?",
            Some("news"),
            &["rust async latest".to_string(), "tokio 2026 updates".to_string()],
            5,
            3,
            1,
        );

        assert!(prompt.contains("What is the latest on Rust async?"));
        assert!(prompt.contains("iteration 2"));
        assert!(prompt.contains("- q1: \"rust async latest\""));
        assert!(prompt.contains("- q2: \"tokio 2026 updates\""));
        assert!(prompt.contains("Vertical used: news"));
        assert!(prompt.contains("total results: 5"));
        assert!(prompt.contains("unique sources: 3"));
    }

    #[test]
    fn build_search_strategy_evaluation_prompt_omits_vertical_when_none() {
        let prompt = build_search_strategy_evaluation_prompt(
            "test",
            None,
            &["query".to_string()],
            1,
            0,
            0,
        );

        assert!(prompt.contains("test"));
        assert!(!prompt.contains("Vertical used:"));
    }

    #[test]
    fn parse_search_strategy_evaluation_parses_valid_json() {
        let raw = r#"{"dimensions": [{"name": "latest news", "attempted": true, "covered": true, "retrieved_count": 5, "query_ids": ["q1"], "status": "covered_strong"}], "missing_dimensions": ["opinions"], "weak_dimensions": [], "recommendation": "escalate_vertical", "reason": "need discussions vertical", "suggested_followup_queries": ["rust async opinions reddit"] }"#;
        let eval = parse_search_strategy_evaluation(raw).unwrap();
        assert_eq!(eval.dimensions.len(), 1);
        assert_eq!(eval.dimensions[0].name, "latest news");
        assert!(eval.dimensions[0].attempted);
        assert!(matches!(eval.dimensions[0].status, DimensionStatus::CoveredStrong));
        assert_eq!(eval.missing_dimensions, vec!["opinions"]);
        assert!(eval.weak_dimensions.is_empty());
        assert!(matches!(eval.recommendation, SearchStrategyRecommendation::EscalateVertical));
        assert_eq!(eval.reason, "need discussions vertical");
        assert_eq!(eval.suggested_followup_queries, vec!["rust async opinions reddit"]);
    }

    #[test]
    fn parse_search_strategy_evaluation_parses_all_recommendations() {
        let synthesize = r#"{"recommendation": "synthesize", "reason": "done"}"#;
        let broaden = r#"{"recommendation": "broaden", "reason": "too few"}"#;
        let escalate = r#"{"recommendation": "escalate_vertical", "reason": "need news"}"#;

        assert!(matches!(
            parse_search_strategy_evaluation(synthesize).unwrap().recommendation,
            SearchStrategyRecommendation::Synthesize
        ));
        assert!(matches!(
            parse_search_strategy_evaluation(broaden).unwrap().recommendation,
            SearchStrategyRecommendation::Broaden
        ));
        assert!(matches!(
            parse_search_strategy_evaluation(escalate).unwrap().recommendation,
            SearchStrategyRecommendation::EscalateVertical
        ));
    }

    #[test]
    fn parse_search_strategy_evaluation_returns_none_for_invalid_json() {
        assert!(parse_search_strategy_evaluation("not json").is_none());
    }
}
