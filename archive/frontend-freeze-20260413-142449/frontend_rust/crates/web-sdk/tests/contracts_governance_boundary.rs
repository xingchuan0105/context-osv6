use contracts::{
    admin::{
        AdminUsageResponse, AuditLogEntry, AuditLogListResponse, AuditLogQuery,
        DegradationStatusResponse, FeatureFlagChangeRequest, FeatureFlagEntry, HealthResponse,
        OrgListResponse, OrgResponse, OrgRow, RagHealthStatus, ReadyResponse, UserListResponse,
        UserRow, WorkerStatusResponse,
    },
    auth::{
        AuthEnvelope, ChangePasswordRequest, ConfirmResetPasswordRequest, LoginRequest,
        NotificationRow, NotificationsResponse, RegisterRequest, SendResetCodeRequest,
        VerifyResetCodeRequest,
    },
    billing::{BillingOverview, PlanRow, PlansResponse, SubscriptionResponse, UsageResponse},
    chat::{ChatMessage, ChatMessageListResponse},
    documents::{
        AnswerContextChunk, CitationLookupRequest, CitationLookupResponse, CreateDocumentRequest,
        CreateDocumentUploadResponse, Document, DocumentContentResponse, DocumentStatusResponse,
        DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse, SourceRow, SourcesResponse,
    },
    notebooks::{
        ApiKeyListResponse, ApiKeyRow, ChatSession, ChatSessionListResponse, CreateApiKeyRequest,
        CreateApiKeyResponse, CreateNotebookRequest, Notebook, NotebookListResponse,
        NotebookResponse, UpdateChatSessionRequest, UpdateNotebookRequest,
    },
    preferences::{DashboardPreferences, UserPreferences, WorkspaceDraftPreference},
    share::{
        AccessLogEntry, AccessLogsResponse, MemberRow, MembersResponse, ShareAnalyticsResponse,
        ShareSettings, ShareTokenResponse, SharedKnowledgeBase, SharedNotebookPayload,
        SharedShareInfo, SharedSource,
    },
};
use web_sdk::dtos as sdk_dtos;

fn same_type<T>(_left: Option<T>, _right: Option<T>) {}

#[test]
fn sdk_uses_contracts_for_active_frontend_models() {
    same_type::<Notebook>(None, None::<sdk_dtos::Notebook>);
    same_type::<NotebookResponse>(None, None::<sdk_dtos::NotebookResponse>);
    same_type::<NotebookListResponse>(None, None::<sdk_dtos::NotebookListResponse>);
    same_type::<CreateNotebookRequest>(None, None::<sdk_dtos::CreateNotebookRequest>);
    same_type::<UpdateNotebookRequest>(None, None::<sdk_dtos::UpdateNotebookRequest>);
    same_type::<ChatSession>(None, None::<sdk_dtos::ChatSession>);
    same_type::<ChatSessionListResponse>(None, None::<sdk_dtos::ChatSessionListResponse>);
    same_type::<UpdateChatSessionRequest>(None, None::<sdk_dtos::UpdateChatSessionRequest>);
    same_type::<ChatMessage>(None, None::<sdk_dtos::ChatMessage>);
    same_type::<ChatMessageListResponse>(None, None::<sdk_dtos::ChatMessageListResponse>);
    same_type::<CreateApiKeyRequest>(None, None::<sdk_dtos::CreateApiKeyRequest>);
    same_type::<CreateApiKeyResponse>(None, None::<sdk_dtos::CreateApiKeyResponse>);
    same_type::<ApiKeyRow>(None, None::<sdk_dtos::ApiKeyRow>);
    same_type::<ApiKeyListResponse>(None, None::<sdk_dtos::ApiKeyListResponse>);
    same_type::<Document>(None, None::<sdk_dtos::Document>);
    same_type::<DocumentsResponse>(None, None::<sdk_dtos::DocumentsResponse>);
    same_type::<CreateDocumentRequest>(None, None::<sdk_dtos::CreateDocumentRequest>);
    same_type::<CreateDocumentUploadResponse>(None, None::<sdk_dtos::CreateDocumentUploadResponse>);
    same_type::<DocumentStatusResponse>(None, None::<sdk_dtos::DocumentStatusResponse>);
    same_type::<DocumentContentResponse>(None, None::<sdk_dtos::DocumentContentResponse>);
    same_type::<ParsedPreviewItem>(None, None::<sdk_dtos::ParsedPreviewItem>);
    same_type::<ParsedPreviewResponse>(None, None::<sdk_dtos::ParsedPreviewResponse>);
    same_type::<SourceRow>(None, None::<sdk_dtos::SourceRow>);
    same_type::<SourcesResponse>(None, None::<sdk_dtos::SourcesResponse>);
    same_type::<CitationLookupRequest>(None, None::<sdk_dtos::CitationLookupRequest>);
    same_type::<CitationLookupResponse>(None, None::<sdk_dtos::CitationLookupResponse>);
    same_type::<AnswerContextChunk>(None, None::<sdk_dtos::AnswerContextChunk>);
    same_type::<AuthEnvelope>(None, None::<sdk_dtos::AuthEnvelope>);
    same_type::<RegisterRequest>(None, None::<sdk_dtos::RegisterRequest>);
    same_type::<LoginRequest>(None, None::<sdk_dtos::LoginRequest>);
    same_type::<ChangePasswordRequest>(None, None::<sdk_dtos::ChangePasswordRequest>);
    same_type::<SendResetCodeRequest>(None, None::<sdk_dtos::SendResetCodeRequest>);
    same_type::<VerifyResetCodeRequest>(None, None::<sdk_dtos::VerifyResetCodeRequest>);
    same_type::<ConfirmResetPasswordRequest>(None, None::<sdk_dtos::ConfirmResetPasswordRequest>);
    same_type::<NotificationRow>(None, None::<sdk_dtos::NotificationRow>);
    same_type::<NotificationsResponse>(None, None::<sdk_dtos::NotificationsResponse>);
    same_type::<DashboardPreferences>(None, None::<sdk_dtos::DashboardPreferences>);
    same_type::<WorkspaceDraftPreference>(None, None::<sdk_dtos::WorkspaceDraftPreference>);
    same_type::<UserPreferences>(None, None::<sdk_dtos::UserPreferences>);
    same_type::<ShareSettings>(None, None::<sdk_dtos::ShareSettings>);
    same_type::<ShareTokenResponse>(None, None::<sdk_dtos::ShareTokenResponse>);
    same_type::<ShareAnalyticsResponse>(None, None::<sdk_dtos::ShareAnalyticsResponse>);
    same_type::<AccessLogEntry>(None, None::<sdk_dtos::AccessLogEntry>);
    same_type::<AccessLogsResponse>(None, None::<sdk_dtos::AccessLogsResponse>);
    same_type::<MemberRow>(None, None::<sdk_dtos::MemberRow>);
    same_type::<MembersResponse>(None, None::<sdk_dtos::MembersResponse>);
    same_type::<SharedKnowledgeBase>(None, None::<sdk_dtos::SharedKnowledgeBase>);
    same_type::<SharedShareInfo>(None, None::<sdk_dtos::SharedShareInfo>);
    same_type::<SharedSource>(None, None::<sdk_dtos::SharedSource>);
    same_type::<SharedNotebookPayload>(None, None::<sdk_dtos::SharedNotebookPayload>);
    same_type::<HealthResponse>(None, None::<sdk_dtos::HealthResponse>);
    same_type::<ReadyResponse>(None, None::<sdk_dtos::ReadyResponse>);
    same_type::<RagHealthStatus>(None, None::<sdk_dtos::RagHealthStatus>);
    same_type::<OrgRow>(None, None::<sdk_dtos::OrgRow>);
    same_type::<OrgListResponse>(None, None::<sdk_dtos::OrgListResponse>);
    same_type::<OrgResponse>(None, None::<sdk_dtos::OrgResponse>);
    same_type::<UserRow>(None, None::<sdk_dtos::UserRow>);
    same_type::<UserListResponse>(None, None::<sdk_dtos::UserListResponse>);
    same_type::<AdminUsageResponse>(None, None::<sdk_dtos::AdminUsageResponse>);
    same_type::<FeatureFlagEntry>(None, None::<sdk_dtos::FeatureFlagEntry>);
    same_type::<FeatureFlagChangeRequest>(None, None::<sdk_dtos::FeatureFlagChangeRequest>);
    same_type::<WorkerStatusResponse>(None, None::<sdk_dtos::WorkerStatusResponse>);
    same_type::<DegradationStatusResponse>(None, None::<sdk_dtos::DegradationStatusResponse>);
    same_type::<AuditLogEntry>(None, None::<sdk_dtos::AuditLogEntry>);
    same_type::<AuditLogQuery>(None, None::<sdk_dtos::AuditLogQuery>);
    same_type::<AuditLogListResponse>(None, None::<sdk_dtos::AuditLogListResponse>);
    same_type::<BillingOverview>(None, None::<sdk_dtos::BillingOverview>);
    same_type::<PlanRow>(None, None::<sdk_dtos::PlanRow>);
    same_type::<PlansResponse>(None, None::<sdk_dtos::PlansResponse>);
    same_type::<SubscriptionResponse>(None, None::<sdk_dtos::SubscriptionResponse>);
    same_type::<UsageResponse>(None, None::<sdk_dtos::UsageResponse>);
}
