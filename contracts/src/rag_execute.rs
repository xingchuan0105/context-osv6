use serde::{Deserialize, Serialize};

use crate::chat::{
    ChatRequest, Citation, DegradeTraceItem, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary,
};
use crate::documents::AnswerContextChunk;

fn default_execute_plan_version() -> String {
    "rag-execute-v1".to_string()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutePlanSummaryMode {
    #[default]
    None,
    Related,
    All,
}

impl ExecutePlanSummaryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Related => "related",
            Self::All => "all",
        }
    }

    fn from_legacy_summary(value: Option<&str>) -> Self {
        match value {
            Some("related") => Self::Related,
            Some("all") => Self::All,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutePlanTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutePlanBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_candidate_budget: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_chunk_budget: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_hop_limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_fan_out_limit: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryEntity {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceholderTriplet {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderTripletType {
    Fuzzy,     // 2+ placeholders
    Traceable, // 1 placeholder
    Resolved,  // 0 placeholders
}

impl PlaceholderTriplet {
    /// Pure position helper (no policy). Runtime code should prefer
    /// `avrag_rag_core::classify_placeholder_triplet`.
    #[deprecated(note = "use avrag_rag_core::classify_placeholder_triplet")]
    pub fn classify(&self) -> PlaceholderTripletType {
        let placeholder_count = self.subject.starts_with('?') as usize
            + self.predicate.starts_with('?') as usize
            + self.object.starts_with('?') as usize;
        match placeholder_count {
            0 => PlaceholderTripletType::Resolved,
            1 => PlaceholderTripletType::Traceable,
            _ => PlaceholderTripletType::Fuzzy,
        }
    }

    /// 返回已知实体（非占位符部分）
    /// 注意：predicate 不是实体，只有 subject 和 object 是实体
    pub fn known_entities(&self) -> Vec<String> {
        let mut entities = Vec::new();
        if !self.subject.starts_with("?") {
            entities.push(self.subject.clone());
        }
        if !self.object.starts_with("?") {
            entities.push(self.object.clone());
        }
        entities
    }

    /// 返回占位符位置
    pub fn placeholder_positions(&self) -> Vec<&str> {
        let mut positions = Vec::new();
        if self.subject.starts_with("?") {
            positions.push("subject");
        }
        if self.predicate.starts_with("?") {
            positions.push("predicate");
        }
        if self.object.starts_with("?") {
            positions.push("object");
        }
        positions
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_dense: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multimodal_dense: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutePlanItem {
    pub priority: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25_terms: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutePlanRequest {
    #[serde(default = "default_execute_plan_version")]
    pub plan_version: String,
    #[serde(default)]
    pub doc_scope: Vec<String>,
    #[serde(default)]
    pub items: Vec<ExecutePlanItem>,
    #[serde(default)]
    pub summary_mode: ExecutePlanSummaryMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<ExecutePlanBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_budget: Option<ChannelBudget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_entities: Vec<QueryEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph_hints: Vec<GraphHint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placeholder_triplets: Vec<PlaceholderTriplet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutePlanTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutePlanValidationError {
    #[error("doc_scope must not be empty")]
    EmptyDocScope,
    #[error("items must not be empty")]
    EmptyItems,
    #[error("items must not contain more than {max} entries")]
    TooManyItems { max: usize },
    #[error("item {index} must contain exactly one payload")]
    InvalidPayloadCount { index: usize },
    #[error("item {index} priority must be between 0.0 and 1.0")]
    InvalidPriority { index: usize },
    #[error("invalid doc_scope: {reason}")]
    InvalidDocScope { reason: String },
    #[error("budget.total_candidate_budget must be greater than zero")]
    InvalidTotalCandidateBudget,
    #[error("budget.final_chunk_budget must be greater than zero")]
    InvalidFinalChunkBudget,
    #[error("placeholder_triplets[{index}] must not contain more than two placeholders")]
    TooManyPlaceholders { index: usize },
    #[error("channel_budget.graph requires graph_hints or placeholder_triplets")]
    GraphBudgetRequiresHints,
}

impl ExecutePlanRequest {
    pub const MAX_ITEMS: usize = 4;

    /// Deprecated: runtime callers must use `avrag_rag_core::validate_execute_plan`.
    ///
    /// Kept only so contracts crate unit tests can exercise the wire schema without
    /// depending on rag-core. Logic is intentionally duplicated and frozen.
    #[deprecated(note = "use avrag_rag_core::validate_execute_plan")]
    #[allow(deprecated)]
    pub fn validate(&self) -> Result<(), ExecutePlanValidationError> {
        // Minimal wire checks only (not the full policy surface). Prefer rag-core.
        if self.doc_scope.is_empty() {
            return Err(ExecutePlanValidationError::EmptyDocScope);
        }
        if self.items.is_empty() {
            return Err(ExecutePlanValidationError::EmptyItems);
        }
        if self.items.len() > Self::MAX_ITEMS {
            return Err(ExecutePlanValidationError::TooManyItems {
                max: Self::MAX_ITEMS,
            });
        }
        for (index, item) in self.items.iter().enumerate() {
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
        if self
            .budget
            .as_ref()
            .and_then(|budget| budget.total_candidate_budget)
            .is_some_and(|value| value == 0)
        {
            return Err(ExecutePlanValidationError::InvalidTotalCandidateBudget);
        }
        if self
            .budget
            .as_ref()
            .and_then(|budget| budget.final_chunk_budget)
            .is_some_and(|value| value == 0)
        {
            return Err(ExecutePlanValidationError::InvalidFinalChunkBudget);
        }
        for (index, triplet) in self.placeholder_triplets.iter().enumerate() {
            if triplet.placeholder_positions().len() > 2 {
                return Err(ExecutePlanValidationError::TooManyPlaceholders { index });
            }
        }
        let graph_budget = self
            .channel_budget
            .as_ref()
            .and_then(|budget| budget.graph)
            .unwrap_or(0);
        if graph_budget > 0 {
            let has_graph = self.graph_hints.iter().any(|hint| {
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
            }) || self.placeholder_triplets.iter().any(|triplet| {
                !triplet.subject.trim().is_empty()
                    || !triplet.predicate.trim().is_empty()
                    || !triplet.object.trim().is_empty()
            });
            if !has_graph {
                return Err(ExecutePlanValidationError::GraphBudgetRequiresHints);
            }
        }
        Ok(())
    }

    /// Deprecated: use `avrag_rag_core::ensure_original_query_text_dense_item`.
    #[deprecated(note = "use avrag_rag_core::ensure_original_query_text_dense_item")]
    pub fn ensure_original_query_text_dense_item(&mut self, original_query: &str) {
        let original_query = original_query.trim();
        if original_query.is_empty() {
            return;
        }
        if self.items.iter().any(|item| {
            item.query
                .as_deref()
                .is_some_and(|query| query.trim() == original_query)
        }) {
            return;
        }
        self.items.insert(
            0,
            ExecutePlanItem {
                priority: 1.0,
                query: Some(original_query.to_string()),
                bm25_terms: None,
            },
        );
        while self.items.len() > Self::MAX_ITEMS {
            self.items.pop();
        }
    }

    pub fn from_rag_plan(plan: &RagPlan, doc_scope: &[String]) -> Self {
        let summary_mode = plan
            .items
            .iter()
            .find_map(|item| item.summary.as_deref())
            .map(|value| ExecutePlanSummaryMode::from_legacy_summary(Some(value)))
            .unwrap_or_default();

        let items = plan
            .items
            .iter()
            .filter_map(|item| {
                let query = item
                    .query
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned);
                let bm25_terms = item.bm25_terms.as_ref().map(|terms| {
                    terms
                        .iter()
                        .map(|term| term.trim())
                        .filter(|term| !term.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                });
                let has_query = query.is_some();
                let has_bm25_terms = bm25_terms.as_ref().is_some_and(|terms| !terms.is_empty());
                (has_query || has_bm25_terms).then_some(ExecutePlanItem {
                    priority: item.priority,
                    query,
                    bm25_terms: bm25_terms.filter(|terms| !terms.is_empty()),
                })
            })
            .collect();

        Self {
            plan_version: plan.plan_version.clone(),
            doc_scope: doc_scope.to_vec(),
            items,
            summary_mode,
            budget: None,
            channel_budget: None,
            query_entities: Vec::new(),
            graph_hints: Vec::new(),
            placeholder_triplets: Vec::new(),
            trace: None,
        }
    }

    /// Extract document IDs from the doc_scope field.
    pub fn doc_ids(&self) -> Option<Vec<uuid::Uuid>> {
        (!self.doc_scope.is_empty()).then(|| {
            self.doc_scope
                .iter()
                .filter_map(|id| uuid::Uuid::parse_str(id).ok())
                .collect::<Vec<_>>()
        })
    }

    /// Deprecated: use `avrag_rag_core::execute_plan_to_chat_request`.
    #[deprecated(note = "use avrag_rag_core::execute_plan_to_chat_request")]
    pub fn to_chat_request_compat(&self) -> ChatRequest {
        let query = self
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
            doc_scope: self.doc_scope.clone(),
            language: None,
            messages: Vec::new(),
            stream: false,
            debug: false,
            format_hint: None,
        }
    }

    pub fn to_rag_plan_compat(&self) -> RagPlan {
        let mut items = self
            .items
            .iter()
            .map(|item| RagPlanItem {
                priority: item.priority,
                query: item.query.clone(),
                bm25_terms: item.bm25_terms.clone(),
                summary: None,
            })
            .collect::<Vec<_>>();
        if self.summary_mode != ExecutePlanSummaryMode::None {
            items.push(RagPlanItem {
                priority: 0.0,
                query: None,
                bm25_terms: None,
                summary: Some(self.summary_mode.as_str().to_string()),
            });
        }
        RagPlan {
            plan_version: self.plan_version.clone(),
            plan_confidence: 1.0,
            clarify_needed: false,
            clarify_message: String::new(),
            items,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreBreakdown {
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub doc_id: String,
    pub chunk_type: String,
    #[serde(default)]
    pub page: Option<i64>,
    pub text: String,
    pub score: f32,
    pub retrieval_channel: String,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub parser_backend: Option<String>,
    #[serde(default)]
    pub source_locator: Option<serde_json::Value>,
    #[serde(default)]
    pub parse_run_id: Option<String>,
    #[serde(default)]
    pub score_breakdown: Vec<ScoreBreakdown>,
}

impl RetrievedChunk {
    pub fn as_answer_context_chunk(&self) -> AnswerContextChunk {
        AnswerContextChunk {
            chunk_id: self.chunk_id.clone(),
            doc_id: Some(self.doc_id.clone()),
            chunk_type: self.chunk_type.clone(),
            page: self.page,
            text: self.text.clone(),
            asset_id: self.asset_id.clone(),
            caption: self.caption.clone(),
            image_url: self.image_url.clone(),
            parser_backend: self.parser_backend.clone(),
            source_locator: self.source_locator.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationPath {
    pub path_id: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub supporting_chunk_ids: Vec<String>,
    pub score: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalBundle {
    #[serde(default)]
    pub chunks: Vec<RetrievedChunk>,
    #[serde(default)]
    pub graph_supported_chunks: Vec<RetrievedChunk>,
    #[serde(default)]
    pub relation_paths: Vec<RelationPath>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub summary_chunks: Vec<AnswerContextChunk>,
}

impl RetrievalBundle {
    pub fn answer_context_chunks(&self) -> Vec<AnswerContextChunk> {
        let mut chunks = self
            .chunks
            .iter()
            .map(RetrievedChunk::as_answer_context_chunk)
            .collect::<Vec<_>>();
        chunks.extend(
            self.graph_supported_chunks
                .iter()
                .map(RetrievedChunk::as_answer_context_chunk),
        );
        chunks.extend(self.summary_chunks.clone());
        chunks
    }

    /// 返回所有可用于 citation 的 chunks，去重并保持 regular chunks 优先
    pub fn citation_chunks(&self) -> Vec<&RetrievedChunk> {
        let mut all_chunks =
            Vec::with_capacity(self.chunks.len() + self.graph_supported_chunks.len());

        // Regular chunks 优先
        all_chunks.extend(&self.chunks);

        // Graph chunks 补充（去重）
        let regular_ids: std::collections::HashSet<_> =
            self.chunks.iter().map(|c| &c.chunk_id).collect();
        for chunk in &self.graph_supported_chunks {
            if !regular_ids.contains(&chunk.chunk_id) {
                all_chunks.push(chunk);
            }
        }

        all_chunks
    }

    /// 检查是否有任何 evidence
    pub fn has_evidence(&self) -> bool {
        !self.chunks.is_empty()
            || !self.graph_supported_chunks.is_empty()
            || !self.summary_chunks.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelCoverage {
    pub text_dense: usize,
    pub bm25: usize,
    pub multimodal_dense: usize,
    pub graph: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub requested_doc_count: usize,
    pub matched_doc_count: usize,
    pub retrieved_chunk_count: usize,
    pub summary_chunk_count: usize,
    #[serde(default)]
    pub channel_coverage: ChannelCoverage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelTraceItem {
    pub channel: String,
    pub raw_count: usize,
    pub hydrated_count: usize,
    pub selected_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutePlanTrace>,
    #[serde(default)]
    pub item_trace: Vec<RagTraceItem>,
    #[serde(default)]
    pub channel_trace: Vec<ChannelTraceItem>,
    pub retrieval_trace: RagTraceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePlanResponse {
    pub bundle: RetrievalBundle,
    pub coverage: Coverage,
    #[serde(default)]
    pub degrade_trace: Vec<DegradeTraceItem>,
    pub backend_trace: BackendTrace,
}
