//! Internal Rust runtime types for avrag services.
//!
//! Re-exports wire DTOs from `contracts` and adds service-only helpers.
//! New cross-language fields belong in `contracts/` first; see `CONTEXT.md`.

pub mod chat;
pub mod content_store;
pub mod docscope;
pub mod documents;
pub mod errors;
pub mod guards_access;
pub mod identity;
pub mod key_vault;
pub mod notebook_requests;
pub mod rag_execute;
pub mod tool_call;
pub mod util;

pub use contracts::chat::{
    AnswerBlock, ChatDonePayload, ChatMessage, ChatMessageListResponse, ChatRequest, ChatResponse,
    ChatTokenUsage, ChatTurnInput, Citation, DegradeReason, DegradeTraceItem, GeneralPlan,
    GuardAction, GuardReport, GuardResult, MessageFeedbackRating, MessageFeedbackRequest, ModeDebug, PlannerOutput,
    RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary, RiskLevel, SearchPlan,
    SourceRef, SummaryInjectionTrace, TraceInfo,
};
pub use contracts::documents::{
    CitationLookupRequest, CitationLookupResponse, CreateDocumentUploadResponse, DocumentStatus,
    DocumentStatusResponse,
};
pub use contracts::notebooks::{
    ChatSession, ChatSessionListResponse, CreateChatSessionRequest, CreateNotebookNoteRequest,
    Notebook, NotebookAnalysisAccess, NotebookAnalysisAlert, NotebookAnalysisNotes,
    NotebookAnalysisOverview, NotebookAnalysisResponse, NotebookAnalysisSources,
    NotebookAnalysisThreads, NotebookListResponse, NotebookNote, NotebookNoteListResponse,
    NotebookNoteResponse, NotebookResponse, PromoteNotebookNoteResponse, UpdateChatSessionRequest,
    UpdateNotebookNoteRequest,
};
pub use contracts::preferences::{
    AgentPreference, AgentPreferenceMemory, BlockedAgentPreference, DailyPreferenceLog,
    DashboardPreferences, NotebookNotePreference, NotebookWorkspacePreference,
    NotificationPreferences, UserPreferences, WorkspaceDraftPreference,
};

pub use chat::{
    answer_blocks_from_rendered_answer, answer_blocks_to_markup, plain_text_answer_blocks,
};
pub use docscope::{
    DocScopeMetadata, DocScopeProfile, Domain, Era, Genre, SummaryMetadata, SummaryOutput,
};
pub use documents::{
    AddUrlSourceRequest, CreateDocumentRequest, Document, DocumentContentResponse,
    DocumentMetadata, DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse,
    SourceRow, SourcesResponse, StatusOnlyResponse, TocEntry, UpdateDocumentRequest,
};
pub use errors::{ApiError, ApiResponse, AppError, ErrorBody};
pub use guards_access::{
    AnswerContextChunk, ApiKeyListResponse, ApiKeyRow, CreateApiKeyRequest, CreateApiKeyResponse,
    InputGuardType, NotificationRow, NotificationsResponse, OutputGuardType, ShareTokenResponse,
};
pub use identity::{OrgId, UserId, default_org_id, default_rag_agent, default_user_id};
pub use notebook_requests::{CreateNotebookRequest, UpdateNotebookRequest};
pub use rag_execute::{
    BackendTrace, ChannelBudget, ChannelCoverage, ChannelTraceItem, Coverage, ExecutePlanBudget,
    ExecutePlanItem, ExecutePlanRequest, ExecutePlanResponse, ExecutePlanSummaryMode,
    ExecutePlanTrace, ExecutePlanValidationError, GraphHint, PlaceholderTriplet, QueryEntity,
    RelationPath, RetrievalBundle, RetrievedChunk, ScoreBreakdown,
};
pub use tool_call::{
    DenseRetrievalArgs, DenseRetrievalModality, DocMetadataArgs, DocProfileArgs, DocSummaryArgs,
    DocSummaryLevel,
    GraphRetrievalArgs, IndexLookupArgs, LexicalRetrievalArgs, MergeConfig, NextStep,
    RetrievalPlannerOutput, RuntimeExecuteRequest, RuntimeExecuteResponse, ToolCall,
    ToolCallAdapterError, ToolResult, ToolSpec, ToolStatus, ToolTrace,
};
pub use content_store::{ContentStore, ContentStoreError, IndexedChunk};
pub use util::{
    estimate_token_count, infer_image_extension, infer_mime_type, is_remote_url, new_id,
    now_rfc3339,
};
