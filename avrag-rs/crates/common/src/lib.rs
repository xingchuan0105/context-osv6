pub mod chat;
pub mod docscope;
pub mod documents;
pub mod errors;
pub mod guards_access;
pub mod health;
pub mod identity;
pub mod notebook_requests;
pub mod rag_execute;
pub mod util;

pub use contracts::chat::{
    AnswerBlock, ChatDonePayload, ChatRequest, ChatResponse, ChatTurnInput, Citation,
    DegradeTraceItem, GeneralPlan, GuardAction, GuardReport, GuardResult, ModeDebug, PlannerOutput,
    RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary, RiskLevel, SearchPlan,
    SourceRef, SummaryInjectionTrace, TraceInfo,
};
pub use contracts::documents::{CreateDocumentUploadResponse, DocumentStatusResponse};
pub use contracts::notebooks::{
    CreateNotebookNoteRequest, Notebook, NotebookAnalysisAccess, NotebookAnalysisAlert,
    NotebookAnalysisNotes, NotebookAnalysisOverview, NotebookAnalysisResponse,
    NotebookAnalysisSources, NotebookAnalysisThreads, NotebookListResponse, NotebookNote,
    NotebookNoteListResponse, NotebookNoteResponse, NotebookResponse, PromoteNotebookNoteResponse,
    UpdateNotebookNoteRequest,
};
pub use contracts::preferences::{
    AgentPreference, AgentPreferenceMemory, BlockedAgentPreference, DailyPreferenceLog,
    DashboardPreferences, NotebookNotePreference, NotebookWorkspacePreference,
    NotificationPreferences, UserPreferences, WorkspaceDraftPreference,
};

pub use chat::{
    ChatMessage, ChatMessageListResponse, ChatSession, ChatSessionListResponse,
    CitationLookupRequest, CitationLookupResponse, CreateChatSessionRequest,
    UpdateChatSessionRequest, answer_blocks_from_rendered_answer, answer_blocks_to_markup,
    plain_text_answer_blocks,
};
pub use docscope::{DocScopeMetadata, DocScopeProfile, SummaryMetadata, SummaryOutput};
pub use documents::{
    AddUrlSourceRequest, CreateDocumentRequest, Document, DocumentContentResponse, DocumentStatus,
    DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse, SourceRow, SourcesResponse,
    StatusOnlyResponse, UpdateDocumentRequest,
};
pub use errors::{ApiError, ApiResponse, AppError, ErrorBody};
pub use guards_access::{
    AnswerContextChunk, ApiKeyListResponse, ApiKeyRow, CreateApiKeyRequest, CreateApiKeyResponse,
    InputGuardType, NotificationRow, NotificationsResponse, OutputGuardType, ShareTokenResponse,
};
pub use health::{HealthResponse, ReadyCheck, ReadyResponse};
pub use identity::{OrgId, UserId, default_org_id, default_rag_agent, default_user_id};
pub use notebook_requests::{CreateNotebookRequest, UpdateNotebookRequest};
pub use rag_execute::{
    BackendTrace, Coverage, ExecutePlanBudget, ExecutePlanItem, ExecutePlanRequest,
    ExecutePlanResponse, ExecutePlanSummaryMode, ExecutePlanTrace, ExecutePlanValidationError,
    RetrievalBundle, RetrievedChunk,
};
pub use util::{
    estimate_token_count, infer_image_extension, infer_mime_type, is_remote_url, new_id,
    now_rfc3339,
};
