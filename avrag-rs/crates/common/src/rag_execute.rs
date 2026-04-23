use serde::{Deserialize, Serialize};

use crate::{
    AnswerContextChunk, ChatRequest, Citation, DegradeTraceItem, RagPlan, RagPlanItem,
    RagTraceItem, RagTraceSummary,
};

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
pub struct ExecutePlanTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutePlanBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_candidate_budget: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_chunk_budget: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutePlanItem {
    pub priority: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25_terms: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub trace: Option<ExecutePlanTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExecutePlanValidationError {
    #[error("items must not be empty")]
    EmptyItems,
    #[error("item {index} must contain exactly one payload")]
    InvalidPayloadCount { index: usize },
    #[error("item {index} priority must be between 0.0 and 1.0")]
    InvalidPriority { index: usize },
    #[error("budget.total_candidate_budget must be greater than zero")]
    InvalidTotalCandidateBudget,
    #[error("budget.final_chunk_budget must be greater than zero")]
    InvalidFinalChunkBudget,
}

impl ExecutePlanRequest {
    pub fn validate(&self) -> Result<(), ExecutePlanValidationError> {
        if self.items.is_empty() {
            return Err(ExecutePlanValidationError::EmptyItems);
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

        Ok(())
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
            trace: None,
        }
    }

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
            messages: Vec::new(),
            stream: false,
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
pub struct RetrievalBundle {
    #[serde(default)]
    pub chunks: Vec<RetrievedChunk>,
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
        chunks.extend(self.summary_chunks.clone());
        chunks
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub requested_doc_count: usize,
    pub matched_doc_count: usize,
    pub retrieved_chunk_count: usize,
    pub summary_chunk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutePlanTrace>,
    #[serde(default)]
    pub item_trace: Vec<RagTraceItem>,
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
