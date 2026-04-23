use anyhow::anyhow;
use contracts::ErrorEnvelope;
#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use std::{error::Error, fmt};

#[derive(Debug, Clone)]
pub struct ApiError {
    status: u16,
    code: Option<String>,
    message: String,
}

impl ApiError {
    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ApiError {}

fn decode_api_error(status: u16, body: &[u8]) -> ApiError {
    if let Ok(error) = serde_json::from_slice::<ErrorEnvelope>(body) {
        return ApiError {
            status,
            code: Some(error.error),
            message: error.message,
        };
    }

    let message = String::from_utf8_lossy(body).trim().to_string();
    ApiError {
        status,
        code: None,
        message: if message.is_empty() {
            format!("Request failed with status {status}")
        } else {
            message
        },
    }
}

/// Base URL for the API (e.g. "http://localhost:8080")
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    #[cfg(not(target_arch = "wasm32"))]
    client: Arc<Client>,
    auth_token: Option<String>,
}

impl ApiClient {
    /// Create a new API client pointing at the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            #[cfg(not(target_arch = "wasm32"))]
            client: Arc::new(Client::new()),
            auth_token: None,
        }
    }

    /// Set the JWT bearer token for authenticated requests.
    pub fn with_auth(self, token: String) -> Self {
        Self {
            auth_token: Some(token),
            ..self
        }
    }

    fn auth_header(&self) -> Option<String> {
        self.auth_token.clone()
    }

    // -------------------------------------------------------------------------
    // Helper: generic request builders
    // -------------------------------------------------------------------------

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        #[cfg(target_arch = "wasm32")]
        {
            let body = self
                .send_wasm_request("GET", path, Option::<&()>::None)
                .await?;
            return Ok(serde_json::from_slice(&body)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}{}", self.base_url, path);
            let mut req = self.client.get(&url);
            if let Some(ref token) = self.auth_header() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = req.send().await?;
            let status = resp.status();
            let body = resp.bytes().await?;
            if !status.is_success() {
                return Err(anyhow!(decode_api_error(status.as_u16(), &body)));
            }
            Ok(serde_json::from_slice(&body)?)
        }
    }

    async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        #[cfg(target_arch = "wasm32")]
        {
            let body = self.send_wasm_request("POST", path, Some(body)).await?;
            return Ok(serde_json::from_slice(&body)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}{}", self.base_url, path);
            let mut req = self.client.post(&url).json(body);
            if let Some(ref token) = self.auth_header() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = req.send().await?;
            let status = resp.status();
            let body = resp.bytes().await?;
            if !status.is_success() {
                return Err(anyhow!(decode_api_error(status.as_u16(), &body)));
            }
            Ok(serde_json::from_slice(&body)?)
        }
    }

    async fn put<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        #[cfg(target_arch = "wasm32")]
        {
            let body = self.send_wasm_request("PUT", path, Some(body)).await?;
            return Ok(serde_json::from_slice(&body)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}{}", self.base_url, path);
            let mut req = self.client.put(&url).json(body);
            if let Some(ref token) = self.auth_header() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = req.send().await?;
            let status = resp.status();
            let body = resp.bytes().await?;
            if !status.is_success() {
                return Err(anyhow!(decode_api_error(status.as_u16(), &body)));
            }
            Ok(serde_json::from_slice(&body)?)
        }
    }

    async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        #[cfg(target_arch = "wasm32")]
        {
            let body = self
                .send_wasm_request("DELETE", path, Option::<&()>::None)
                .await?;
            return Ok(serde_json::from_slice(&body)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}{}", self.base_url, path);
            let mut req = self.client.delete(&url);
            if let Some(ref token) = self.auth_header() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = req.send().await?;
            let status = resp.status();
            let body = resp.bytes().await?;
            if !status.is_success() {
                return Err(anyhow!(decode_api_error(status.as_u16(), &body)));
            }
            Ok(serde_json::from_slice(&body)?)
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn send_wasm_request<B: Serialize>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> anyhow::Result<Vec<u8>> {
        use gloo_net::http::Request;

        let url = format!("{}{}", self.base_url, path);
        let mut request = match method {
            "GET" => Request::get(&url),
            "POST" => Request::post(&url),
            "PUT" => Request::put(&url),
            "DELETE" => Request::delete(&url),
            _ => anyhow::bail!("unsupported wasm request method: {method}"),
        };
        if let Some(token) = self.auth_header() {
            request = request.header("Authorization", &format!("Bearer {}", token));
        }
        let response = if let Some(body) = body {
            request
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(body)?)?
                .send()
                .await?
        } else {
            request.send().await?
        };
        if !response.ok() {
            let status = response.status();
            let body = response.binary().await?;
            return Err(anyhow!(decode_api_error(status, &body)));
        }
        Ok(response.binary().await?)
    }
}

// ---------------------------------------------------------------------------
// DTO re-exports (mirrors of types in `common` crate)
// ---------------------------------------------------------------------------

pub mod dtos {
    pub use contracts::admin::{
        AdminUsageResponse, AuditLogEntry, AuditLogListResponse, AuditLogQuery,
        DegradationStatusResponse, FeatureFlagChangeRequest, FeatureFlagEntry, HealthResponse,
        OrgListResponse, OrgResponse, OrgRow, RagHealthStatus, ReadyResponse, UserListResponse,
        UserRow, WorkerStatusResponse,
    };
    pub use contracts::auth::{
        AuthEnvelope, AuthPayload, AuthRuntimeCapabilitiesResponse, AuthUserDto,
        ChangePasswordRequest, ConfirmResetPasswordRequest, EmptyResponse, LoginRequest,
        NotificationRow, NotificationsResponse, RegisterRequest, SendResetCodeRequest,
        VerifyResetCodeRequest,
    };
    pub use contracts::chat::{
        AnswerBlock, ChatDonePayload, ChatMessage, ChatMessageListResponse, ChatRequest,
        ChatResponse, ChatTurnInput, Citation, DegradeTraceItem, GeneralPlan, GuardReport, ModeDebug,
        PlannerOutput, RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary,
        SearchPlan, SourceRef, SummaryInjectionTrace, TraceInfo,
    };
    pub use contracts::documents::{
        AnswerContextChunk, CitationLookupRequest, CitationLookupResponse, CreateDocumentRequest,
        CreateDocumentUploadResponse, Document, DocumentContentResponse, DocumentStatusResponse,
        DocumentsResponse, ParsedPreviewItem, ParsedPreviewResponse, SourceRow, SourcesResponse,
    };
    pub use contracts::billing::{
        BillingOverview, PlanRow, PlansResponse, SubscriptionResponse, UsageResponse,
    };
    pub use contracts::notebooks::{
        ApiKeyListResponse, ApiKeyRow, ChatSession, ChatSessionListResponse, CreateApiKeyRequest,
        CreateApiKeyResponse, CreateChatSessionRequest, CreateNotebookNoteRequest,
        CreateNotebookRequest, Notebook, NotebookAnalysisAccess, NotebookAnalysisAlert,
        NotebookAnalysisNotes, NotebookAnalysisOverview, NotebookAnalysisResponse,
        NotebookAnalysisSources, NotebookAnalysisThreads, NotebookListResponse, NotebookNote,
        NotebookNoteListResponse, NotebookNoteResponse, NotebookResponse,
        PromoteNotebookNoteResponse, UpdateChatSessionRequest, UpdateNotebookNoteRequest,
        UpdateNotebookRequest,
    };
    pub use contracts::preferences::{
        DashboardPreferences, NotebookNotePreference, NotebookWorkspacePreference,
        NotificationPreferences, UserPreferences, WorkspaceDraftPreference,
    };
    pub use contracts::share::{
        AccessLogEntry, AccessLogsResponse, MemberRow, MembersResponse, ShareAnalyticsResponse,
        ShareSettings, ShareTokenResponse, SharedKnowledgeBase, SharedNotebookPayload,
        SharedShareInfo, SharedSource,
    };
}

// Module declarations for API client modules
