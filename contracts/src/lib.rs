pub mod admin;
pub mod agent_permissions;
pub mod auth;
pub mod auth_runtime;
pub mod billing;
pub mod chat;
pub mod documents;
pub mod errors;
pub mod workspaces;
pub mod preferences;
pub mod rag_execute;
pub mod share;
pub mod tool_call;
pub mod usage_limit;

pub use admin::{
    AdminUsageResponse, AuditLogEntry, AuditLogListResponse, AuditLogQuery,
    DegradationStatusResponse, FeatureFlagChangeRequest, FeatureFlagEntry, HealthResponse,
    OrgListResponse, OrgResponse, OrgRow, RagHealthStatus, ReadyResponse, UserListResponse,
    UserRow, WorkerStatusResponse,
};
pub use agent_permissions::{
    ORG_KEY_DEFAULT_PERMISSIONS, PERM_ADMIN, PERM_INDEX, PERM_QUERY, PERM_WORKSPACE_CREATE,
    PERM_WORKSPACE_LIST, USER_ROLE_ORG_ADMIN, WORKSPACE_KEY_DEFAULT_PERMISSIONS,
    normalize_api_key_permissions, user_role_grants_org_admin,
};
pub use auth::{AuthEnvelope, AuthPayload, AuthRuntimeCapabilitiesResponse, AuthUserDto};
pub use auth::{
    ChangePasswordRequest, ConfirmResetPasswordRequest, EmptyResponse, LoginRequest,
    NotificationRow, NotificationsResponse, RegisterRequest, SendResetCodeRequest,
    VerifyResetCodeRequest,
};
pub use auth_runtime::{
    ActorId, AuthContext, AuthError, SubjectKind, UserId, ensure_same_owner,
};
pub use billing::{BillingOverview, PlanRow, PlansResponse, SubscriptionResponse, UsageResponse};
pub use chat::{
    AnswerBlock, ChatDonePayload, ChatEvent, ChatMessage, ChatMessageListResponse, ChatRequest,
    ChatResponse, ChatTurnInput, Citation, DegradeTraceItem, GeneralPlan, GuardAction, GuardReport,
    GuardResult, MessageFeedbackRating, MessageFeedbackRequest, ModeDebug, PlannerOutput,
    RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary, RiskLevel, SearchPlan,
    SourceRef, SummaryInjectionTrace, ToolResult, ToolStatus, ToolTrace, TraceInfo,
};
pub use documents::{
    AnswerContextChunk, CitationLookupRequest, CitationLookupResponse, CreateDocumentRequest,
    CreateDocumentUploadResponse, Document, DocumentContentResponse, DocumentStatus,
    DocumentStatusResponse, DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse, SourceRow,
    SourcesResponse,
};
pub use errors::ErrorEnvelope;
pub use workspaces::{
    ApiKeyListResponse, ApiKeyRow, ChatSession, ChatSessionListResponse, CreateApiKeyRequest,
    CreateApiKeyResponse, CreateChatSessionRequest, CreateWorkspaceNoteRequest,
    CreateWorkspaceRequest, Workspace, WorkspaceAnalysisAccess, WorkspaceAnalysisAlert,
    WorkspaceAnalysisNotes, WorkspaceAnalysisOverview, WorkspaceAnalysisResponse,
    WorkspaceAnalysisSources, WorkspaceAnalysisThreads, WorkspaceListResponse, WorkspaceNote,
    WorkspaceNoteListResponse, WorkspaceNoteResponse, WorkspaceResponse, PromoteWorkspaceNoteResponse,
    UpdateChatSessionRequest, UpdateWorkspaceNoteRequest, UpdateWorkspaceRequest,
};
pub use preferences::{
    AgentPreference, AgentPreferenceMemory, BlockedAgentPreference, DailyPreferenceLog,
    DashboardPreferences, WorkspaceNotePreference, WorkspacePreference,
    NotificationPreferences, UserPreferences, WorkspaceDraftPreference,
};
pub use rag_execute::{
    BackendTrace, ChannelCoverage, ChannelTraceItem, Coverage, GraphHint, PlaceholderTriplet,
    PlaceholderTripletType, QueryEntity, RelationPath, RetrievalBundle, RetrievedChunk,
    ScoreBreakdown,
};
pub use share::{
    AccessLogEntry, AccessLogsResponse, MemberRow, MembersResponse, ShareAnalyticsResponse,
    ShareSettings, ShareTokenResponse, SharedKnowledgeBase, SharedWorkspacePayload, SharedShareInfo,
    SharedSource,
};
pub use tool_call::{
    DenseRetrievalArgs, DenseRetrievalModality, DocChunksArgs, DocMetadataArgs, DocProfileArgs,
    DocSummaryArgs, DocSummaryLevel, GraphRetrievalArgs, IndexLookupArgs, LexicalRetrievalArgs,
    MergeConfig, NextStep, RetrievalPlannerOutput, RuntimeExecuteRequest, RuntimeExecuteResponse,
    ToolCall, ToolSpec,
};
pub use usage_limit::{
    UsageLimitPolicy, UsageLimitResponse, UsageScope, UsageWindow, UsageWindows,
};
