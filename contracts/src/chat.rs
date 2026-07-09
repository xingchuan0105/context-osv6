use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_chat_agent")]
    pub agent_type: String,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_token: Option<String>,
    #[serde(default)]
    pub doc_scope: Vec<String>,
    #[serde(default)]
    pub messages: Vec<ChatTurnInput>,
    #[serde(default)]
    pub stream: bool,
    /// When true, emit debug trace events (e.g. prompt snapshots) in SSE streams.
    #[serde(default)]
    pub debug: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_hint: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTurnInput {
    pub role: String,
    pub content: String,
    /// Prior-turn resolved query from PG `turn_metadata` when available.
    #[serde(default)]
    pub resolved_query: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    #[typeshare(serialized_as = "number")]
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub answer_blocks: Vec<AnswerBlock>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub agent_icon: Option<String>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub tool_results: Vec<ToolResult>,
    #[serde(default)]
    pub turn_metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_query: Option<String>,
    pub created_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageListResponse {
    pub messages: Vec<ChatMessage>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    #[typeshare(serialized_as = "number")]
    pub citation_id: i64,
    pub doc_id: String,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub page: Option<usize>,
    pub doc_name: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    pub score: f32,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub chunk_type: Option<String>,
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
}

#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(
    export,
    export_to = "../../frontend_next/lib/contracts/generated/answer_block.ts"
)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AnswerBlock {
    Text {
        text: String,
        #[serde(default)]
        citations: Vec<String>,
    },
    Image {
        chunk_id: String,
    },
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub doc_id: Option<String>,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub page: Option<usize>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceInfo {
    pub mode: String,
}

/// Serializes as a stable snake_case string on the wire.
#[typeshare(serialized_as = "String")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradeReason {
    BudgetExhausted,
    NoResultsAfterAllFallbacks,
    AllToolsFailed,
    ProviderUnavailable,
    EmbeddingUnavailable,
    LexicalFallback,
    Search429,
    SearchTimeout,
    EmptyDocument,
    NoReadyDocumentContext,
    NoValidRetrievalResults,
    NoRetrievalEvidence,
    ContentGuard,
    ChannelTimeout,
    ChannelFailed,
    ToolDegraded,
    ToolUnavailable,
    PlannerFailed,
    Other(String),
}

impl DegradeReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::BudgetExhausted => "budget_exhausted",
            Self::NoResultsAfterAllFallbacks => "no_results_after_all_fallbacks",
            Self::AllToolsFailed => "all_tools_failed",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::EmbeddingUnavailable => "embedding_unavailable",
            Self::LexicalFallback => "lexical_fallback",
            Self::Search429 => "search_429",
            Self::SearchTimeout => "search_timeout",
            Self::EmptyDocument => "empty_document",
            Self::NoReadyDocumentContext => "no_ready_document_context",
            Self::NoValidRetrievalResults => "no_valid_retrieval_results",
            Self::NoRetrievalEvidence => "no_retrieval_evidence",
            Self::ContentGuard => "content_guard",
            Self::ChannelTimeout => "channel_timeout",
            Self::ChannelFailed => "channel_failed",
            Self::ToolDegraded => "tool_degraded",
            Self::ToolUnavailable => "tool_unavailable",
            Self::PlannerFailed => "planner_failed",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "budget_exhausted" => Self::BudgetExhausted,
            "no_results" | "no_results_after_all_fallbacks" => Self::NoResultsAfterAllFallbacks,
            "all_tools_failed" => Self::AllToolsFailed,
            "provider_unavailable" => Self::ProviderUnavailable,
            "embedding_unavailable" => Self::EmbeddingUnavailable,
            "lexical_fallback" => Self::LexicalFallback,
            "search_429" => Self::Search429,
            "search_timeout" => Self::SearchTimeout,
            "empty_document" => Self::EmptyDocument,
            "no_ready_document_context" => Self::NoReadyDocumentContext,
            "no_valid_retrieval_results" => Self::NoValidRetrievalResults,
            "no_retrieval_evidence" => Self::NoRetrievalEvidence,
            "content_guard" => Self::ContentGuard,
            "channel_timeout" => Self::ChannelTimeout,
            "channel_failed" => Self::ChannelFailed,
            "tool_degraded" => Self::ToolDegraded,
            "tool_unavailable" => Self::ToolUnavailable,
            "planner_failed" => Self::PlannerFailed,
            other => Self::Other(other.to_string()),
        }
    }

    /// Stable stage identifier used in activity events (legacy react_loop helper).
    pub fn as_stage(&self) -> &'static str {
        match self {
            Self::BudgetExhausted => "budget_exhausted",
            Self::NoResultsAfterAllFallbacks => "no_results",
            Self::AllToolsFailed => "all_tools_failed",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::Other(_) => "other",
            _ => "degraded",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::BudgetExhausted => "iteration budget exhausted".to_string(),
            Self::NoResultsAfterAllFallbacks => {
                "no results after broadening query variants".to_string()
            }
            Self::AllToolsFailed => "all tool calls failed".to_string(),
            Self::ProviderUnavailable => "provider unavailable".to_string(),
            Self::NoRetrievalEvidence => {
                "no retrieval evidence after loop and fallback".to_string()
            }
            Self::EmbeddingUnavailable => "embedding service unavailable".to_string(),
            Self::Other(msg) => msg.clone(),
            other => other.as_str().replace('_', " "),
        }
    }
}

impl Serialize for DegradeReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DegradeReason {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from_str(&value))
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradeTraceItem {
    pub stage: String,
    pub reason: DegradeReason,
    pub impact: String,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardAction {
    Allow,
    Block,
    Truncate,
    Redact,
    Flag,
}

impl std::fmt::Display for GuardAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardAction::Allow => write!(f, "allow"),
            GuardAction::Block => write!(f, "block"),
            GuardAction::Truncate => write!(f, "truncate"),
            GuardAction::Redact => write!(f, "redact"),
            GuardAction::Flag => write!(f, "flag"),
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardResult {
    pub passed: bool,
    pub guard_type: String,
    pub risk_level: RiskLevel,
    pub action: GuardAction,
    pub reason: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

impl GuardResult {
    pub fn pass(guard_type: &str) -> Self {
        Self {
            passed: true,
            guard_type: guard_type.to_string(),
            risk_level: RiskLevel::Low,
            action: GuardAction::Allow,
            reason: String::new(),
            trace_id: None,
            operator: None,
            details: None,
        }
    }

    pub fn block(
        guard_type: &str,
        risk_level: RiskLevel,
        reason: impl Into<String>,
        trace_id: Option<String>,
        operator: Option<String>,
    ) -> Self {
        Self {
            passed: false,
            guard_type: guard_type.to_string(),
            risk_level,
            action: GuardAction::Block,
            reason: reason.into(),
            trace_id,
            operator,
            details: None,
        }
    }

    pub fn redact(guard_type: &str, reason: impl Into<String>, details: serde_json::Value) -> Self {
        Self {
            passed: true,
            guard_type: guard_type.to_string(),
            risk_level: RiskLevel::Medium,
            action: GuardAction::Redact,
            reason: reason.into(),
            trace_id: None,
            operator: None,
            details: Some(details),
        }
    }

    pub fn flag(guard_type: &str, risk_level: RiskLevel, reason: impl Into<String>) -> Self {
        Self {
            passed: true,
            guard_type: guard_type.to_string(),
            risk_level,
            action: GuardAction::Flag,
            reason: reason.into(),
            trace_id: None,
            operator: None,
            details: None,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuardReport {
    #[serde(default)]
    pub input_results: Vec<GuardResult>,
    #[serde(default)]
    pub output_results: Vec<GuardResult>,
    pub blocked: bool,
    #[serde(default)]
    pub degrade_trace: Vec<DegradeTraceItem>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagPlan {
    #[serde(default = "default_rag_plan_version")]
    pub plan_version: String,
    #[serde(default = "default_rag_plan_confidence")]
    pub plan_confidence: f32,
    #[serde(default)]
    pub clarify_needed: bool,
    #[serde(default)]
    pub clarify_message: String,
    #[serde(default)]
    pub items: Vec<RagPlanItem>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagPlanItem {
    pub priority: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm25_terms: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerOutput {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rag_plan: Option<RagPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_plan: Option<SearchPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general_plan: Option<GeneralPlan>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPlan {
    pub query_type: String,
    pub sub_queries: Vec<String>,
    pub source_requirements: String,
    pub output_format: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralPlan {
    pub context_trimming_plan: String,
    pub style_constraints: String,
    pub output_format: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagTraceSummary {
    #[typeshare(serialized_as = "number")]
    pub item_count: usize,
    #[typeshare(serialized_as = "number")]
    pub total_candidate_budget: usize,
    #[typeshare(serialized_as = "number")]
    pub max_rerank_docs: usize,
    #[typeshare(serialized_as = "number")]
    pub max_final_chunks: usize,
    #[typeshare(serialized_as = "number")]
    pub top_k_returned: usize,
    pub summary_mode: String,
    pub items: Vec<RagTraceItem>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagTraceItem {
    pub priority: f32,
    pub payload_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default)]
    pub bm25_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[typeshare(serialized_as = "number")]
    pub recall_budget: usize,
    #[typeshare(serialized_as = "number")]
    pub bm25_k: usize,
    #[typeshare(serialized_as = "number")]
    pub dense_k: usize,
    #[typeshare(serialized_as = "number")]
    pub rerank_budget: usize,
    #[typeshare(serialized_as = "number")]
    pub source_count: usize,
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeDebug {
    #[serde(default)]
    pub rag: Option<RagModeDebug>,
    #[serde(default)]
    pub search: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    pub general: Option<BTreeMap<String, serde_json::Value>>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagModeDebug {
    pub item_trace: Vec<RagTraceItem>,
    pub retrieval_trace: RagTraceSummary,
    pub summary_injection_trace: SummaryInjectionTrace,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryInjectionTrace {
    pub mode: String,
    #[typeshare(serialized_as = "number")]
    pub injected_count: usize,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    Timeout,
    Error,
    NotFound,
    NotImplemented,
}

/// Per-tool execution trace (latency, hit counts, degrade reason).
#[typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(serialized_as = "number")]
    pub elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(serialized_as = "number")]
    pub raw_hit_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[typeshare(serialized_as = "number")]
    pub hydrated_hit_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub version: String,
    pub status: ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ToolTrace>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTokenUsage {
    #[typeshare(serialized_as = "number")]
    pub prompt_tokens: u64,
    #[typeshare(serialized_as = "number")]
    pub completion_tokens: u64,
    #[typeshare(serialized_as = "number")]
    pub total_tokens: u64,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub cached_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub answer: String,
    #[serde(default)]
    pub answer_blocks: Vec<AnswerBlock>,
    pub session_id: String,
    pub agent_type: String,
    pub sources: Vec<SourceRef>,
    pub citations: Vec<Citation>,
    pub trace: TraceInfo,
    pub degrade_trace: Vec<DegradeTraceItem>,
    #[serde(default)]
    pub planner_output: Option<PlannerOutput>,
    #[serde(default)]
    pub mode_debug: Option<ModeDebug>,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub message_id: Option<i64>,
    #[serde(default)]
    pub guard_report: Option<GuardReport>,
    #[serde(default)]
    pub tool_results: Vec<ToolResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatTokenUsage>,
    /// Per-invocation instructions for external agents (RAG codegen / Search tool schema).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_operation_guide: Option<AgentOperationGuide>,
}

#[typeshare]
#[derive(TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(
    export,
    export_to = "../../frontend_next/lib/contracts/generated/agent_operation_guide.ts"
)]
pub struct AgentOperationGuide {
    pub mode: String,
    pub summary: String,
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(type = "Array<Record<string, unknown>>")]
    pub tool_schemas: Vec<crate::tool_call::ToolSpec>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDonePayload {
    pub request_id: String,
    pub session_id: String,
    #[typeshare(serialized_as = "number")]
    pub message_id: i64,
    pub response: ChatResponse,
}

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatActivitySourcePreview {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub href: Option<String>,
}

/// Chat event JSON contract for the converged chat protocol.
///
/// SSE framing stays a transport concern handled by the HTTP layer.
#[derive(TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(
    export,
    export_to = "../../frontend_next/lib/contracts/generated/chat_event.ts"
)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatEvent {
    Start {
        request_id: String,
        session_id: String,
    },
    OperationGuide {
        request_id: String,
        guide: AgentOperationGuide,
    },
    Activity {
        request_id: String,
        phase: String,
        title: String,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        #[ts(type = "Record<string, number>")]
        counts: BTreeMap<String, usize>,
        #[serde(default)]
        #[ts(type = "Array<{ id: string; label: string; href?: string | null }>")]
        sources_preview: Vec<ChatActivitySourcePreview>,
        #[serde(default)]
        timestamp: Option<String>,
    },
    AnswerStart {
        request_id: String,
        session_id: String,
        #[ts(type = "number")]
        message_id: i64,
        agent_type: String,
    },
    Trace {
        request_id: String,
        stage: String,
        status: String,
        #[serde(default)]
        #[ts(type = "unknown")]
        detail: Option<serde_json::Value>,
    },
    Token {
        request_id: String,
        #[ts(type = "number")]
        message_id: i64,
        content: String,
    },
    ReasoningSummaryDelta {
        request_id: String,
        #[ts(type = "number")]
        message_id: i64,
        content: String,
    },
    Citations {
        request_id: String,
        #[ts(type = "number")]
        message_id: i64,
        #[ts(type = "Array<Record<string, unknown>>")]
        citations: Vec<serde_json::Value>,
    },
    Done {
        request_id: String,
        session_id: String,
        #[ts(type = "number")]
        message_id: i64,
        #[ts(type = "Record<string, unknown>")]
        payload: serde_json::Value,
    },
    Error {
        request_id: String,
        code: String,
        message: String,
    },
}

fn default_chat_agent() -> String {
    "chat".to_string()
}

fn default_rag_plan_version() -> String {
    "rag-item-v2".to_string()
}

fn default_rag_plan_confidence() -> f32 {
    0.5
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageFeedbackRating {
    Up,
    Down,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFeedbackRequest {
    pub session_id: String,
    #[typeshare(serialized_as = "number")]
    pub message_id: i64,
    pub rating: MessageFeedbackRating,
}
