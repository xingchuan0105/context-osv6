use common::{
    ChatMessage, ChatRequest,
    Citation, DegradeTraceItem, DocumentStatus,
    ModeDebug, ParsedPreviewItem, PlannerOutput,
    RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary,
    SourceRef, SummaryInjectionTrace,
};

use crate::lib_impl::*;

pub(crate) fn next_message_id(state: &mut MemoryState) -> i64 {
    state.next_message_id += 1;
    state.next_message_id
}

pub fn status_label(status: &DocumentStatus) -> &'static str {
    match status {
        DocumentStatus::Pending => "pending",
        DocumentStatus::Enqueueing => "enqueueing",
        DocumentStatus::Queued => "queued",
        DocumentStatus::Processing => "processing",
        DocumentStatus::Completed => "completed",
        DocumentStatus::Failed => "failed",
        DocumentStatus::Deleting => "deleting",
        DocumentStatus::Deleted => "deleted",
        DocumentStatus::UploadInvalid => "upload_invalid",
    }
}

pub fn derive_profile_domains(messages: &[ChatMessage], query: &str) -> Vec<String> {
    let corpus = messages
        .iter()
        .map(|m| m.content.as_str())
        .chain(std::iter::once(query))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let mut domains = Vec::new();
    if corpus.contains("rust") || corpus.contains("code") || corpus.contains("api") {
        domains.push("software".to_string());
    }
    if corpus.contains("contract") || corpus.contains("policy") || corpus.contains("regulation") {
        domains.push("policy".to_string());
    }
    if domains.is_empty() {
        domains.push("general".to_string());
    }
    domains
}

pub fn derive_profile_topics(messages: &[ChatMessage], query: &str) -> Vec<String> {
    messages
        .iter()
        .map(|m| m.content.trim())
        .chain(std::iter::once(query.trim()))
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn detect_preferred_style(query: &str) -> Option<String> {
    let normalized = query.to_ascii_lowercase();
    if normalized.contains("brief") || normalized.contains("concise") || normalized.contains("??") {
        Some("concise".to_string())
    } else if normalized.contains("detail") || normalized.contains("??") {
        Some("detailed".to_string())
    } else {
        None
    }
}

pub fn merge_general_profile_custom_preferences(
    mut custom_preferences: serde_json::Value,
    agent_memory: common::AgentPreferenceMemory,
    query: &str,
    refined_query: &str,
) -> serde_json::Value {
    if !custom_preferences.is_object() {
        custom_preferences = serde_json::json!({});
    }
    if let Some(object) = custom_preferences.as_object_mut() {
        object.entry("agent_memory".to_string()).or_insert_with(|| {
            serde_json::to_value(agent_memory).unwrap_or_else(|_| serde_json::json!({}))
        });
        object.insert(
            "last_general_query".to_string(),
            serde_json::json!(query.trim()),
        );
        object.insert(
            "refined_query".to_string(),
            serde_json::json!(refined_query.trim()),
        );
    }
    custom_preferences
}

pub fn build_degrade_trace(agent_type: &str, has_context: bool) -> Vec<DegradeTraceItem> {
    if has_context {
        Vec::new()
    } else {
        vec![DegradeTraceItem {
            stage: format!("{}.fallback", agent_type),
            reason: "no_ready_document_context".to_string(),
            impact: "Used a fallback response without grounded document context".to_string(),
        }]
    }
}

pub fn build_summary(content: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.chars().take(180).collect()
}

pub fn build_parsed_preview(content: &str) -> Vec<ParsedPreviewItem> {
    let mut items = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        items.push(ParsedPreviewItem {
            kind: "paragraph".to_string(),
            text: trimmed.to_string(),
            page: 1,
            cursor: index,
        });
    }
    if items.is_empty() {
        items.push(ParsedPreviewItem {
            kind: "paragraph".to_string(),
            text: "Document uploaded but no previewable text was extracted.".to_string(),
            page: 1,
            cursor: 0,
        });
    }
    items
}

pub(crate) fn build_answer(
    query: &str,
    agent_type: &str,
    notebook_name: &str,
    context_document: Option<&StoredDocument>,
    ready_document_count: usize,
) -> String {
    match agent_type {
        "general" => format!(
            "M1/M2 skeleton general response for \"{query}\". Multi-turn memory and production LLM routing are not connected yet."
        ),
        "search" => format!(
            "M1/M2 skeleton search response for \"{query}\". Web search retrieval is not connected yet, so this is a contract-validation response."
        ),
        _ => {
            if let Some(document) = context_document {
                let preview: String = document.content.chars().take(220).collect();
                return format!(
                    "M1/M2 skeleton RAG response for notebook \"{notebook_name}\". I found {ready_document_count} ready document(s). Placeholder context came from \"{}\": {}",
                    document.document.file_name, preview
                );
            }
            format!(
                "当前知识库证据不足。M1/M2 skeleton 已建立 chat/notebook/document 主链路，但真实 RAG 检索尚未接入，无法基于私有资料回答：{query}"
            )
        }
    }
}

pub(crate) fn build_citations(context_document: Option<&RetrievedContext>) -> Vec<Citation> {
    let Some(document) = context_document else {
        return Vec::new();
    };
    vec![Citation {
        citation_id: 1,
        doc_id: document.stored_document.document.id.clone(),
        chunk_id: Some(document.chunk_id.clone()),
        page: document.page,
        doc_name: document.stored_document.document.file_name.clone(),
        preview: Some(document.stored_document.content.chars().take(140).collect()),
        content: Some(document.stored_document.content.clone()),
        score: document.score,
        layer: Some("chunk".to_string()),
        chunk_type: Some("text".to_string()),
        asset_id: None,
        caption: None,
        image_url: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: None,
    }]
}

pub(crate) fn build_sources(context_document: Option<&RetrievedContext>) -> Vec<SourceRef> {
    let Some(document) = context_document else {
        return Vec::new();
    };
    vec![SourceRef {
        id: document.chunk_id.clone(),
        title: document.stored_document.document.file_name.clone(),
        snippet: Some(document.stored_document.content.chars().take(160).collect()),
        doc_id: Some(document.stored_document.document.id.clone()),
        page: document.page,
    }]
}

pub(crate) fn build_planner_output(
    req: &ChatRequest,
    retrieval: Option<&RetrievedContext>,
) -> Option<PlannerOutput> {
    if req.agent_type != "rag" {
        return None;
    }
    let mut items = vec![RagPlanItem {
        priority: 0.8,
        query: Some(req.query.clone()),
        bm25_terms: None,
        summary: None,
    }];
    if let Some(keyword) = extract_keyword_hint(&req.query) {
        items.push(RagPlanItem {
            priority: 0.2,
            query: None,
            bm25_terms: Some(vec![keyword]),
            summary: None,
        });
    }
    Some(PlannerOutput {
        mode: "rag".to_string(),
        rag_plan: Some(RagPlan {
            plan_version: "rag-item-v2".to_string(),
            plan_confidence: if retrieval.is_some() { 0.72 } else { 0.35 },
            clarify_needed: false,
            clarify_message: String::new(),
            items,
        }),
        search_plan: None,
        general_plan: None,
    })
}

pub(crate) fn build_mode_debug(
    req: &ChatRequest,
    retrieval: Option<&RetrievedContext>,
    sources: &[SourceRef],
) -> Option<ModeDebug> {
    if req.agent_type != "rag" {
        return None;
    }

    let trace_item = RagTraceItem {
        priority: 0.8,
        payload_kind: "query".to_string(),
        query: Some(req.query.clone()),
        bm25_terms: Vec::new(),
        summary: None,
        recall_budget: 8,
        bm25_k: retrieval.map(|item| item.sparse_hits).unwrap_or(0),
        dense_k: retrieval.map(|item| item.dense_hits).unwrap_or(0),
        rerank_budget: 4,
        source_count: retrieval
            .map(|item| item.source_count)
            .unwrap_or(sources.len()),
        source_ids: retrieval
            .map(|item| item.source_ids.clone())
            .unwrap_or_else(|| sources.iter().map(|source| source.id.clone()).collect()),
    };

    let retrieval_trace = RagTraceSummary {
        item_count: 1,
        total_candidate_budget: 8,
        max_rerank_docs: 4,
        max_final_chunks: sources.len(),
        top_k_returned: sources.len(),
        summary_mode: "none".to_string(),
        items: vec![trace_item.clone()],
    };

    Some(ModeDebug {
        rag: Some(RagModeDebug {
            item_trace: vec![trace_item],
            retrieval_trace,
            summary_injection_trace: SummaryInjectionTrace {
                mode: "none".to_string(),
                injected_count: 0,
            },
        }),
        search: None,
        general: None,
    })
}

pub fn estimate_token_count(text: &str) -> i64 {
    common::estimate_token_count(text)
}

pub fn extract_keyword_hint(query: &str) -> Option<String> {
    let quoted = query
        .split('"')
        .nth(1)
        .map(str::trim)
        .filter(|value| value.split_whitespace().count() >= 2)
        .map(ToOwned::to_owned);
    if quoted.is_some() {
        return quoted;
    }

    query
        .split(':')
        .nth(1)
        .map(str::trim)
        .filter(|value| value.split_whitespace().count() >= 2)
        .map(ToOwned::to_owned)
}

pub fn agent_name(agent_type: &str, language: Option<&str>) -> &'static str {
    crate::chat::i18n::agent_name(agent_type, language)
}

pub fn agent_icon(agent_type: &str) -> &'static str {
    match agent_type {
        "search" => "🔍",
        "general" => "💬",
        _ => "📚",
    }
}
