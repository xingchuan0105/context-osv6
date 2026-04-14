use contracts::{
    admin::HealthResponse,
    auth::{AuthEnvelope, LoginRequest, RegisterRequest},
    billing::UsageResponse,
    chat::{ChatEvent, ChatRequest, ChatResponse},
    documents::{CreateDocumentRequest, DocumentsResponse},
    notebooks::{CreateNotebookRequest, NotebookResponse},
    share::ShareTokenResponse,
    usage_limit::UsageLimitResponse,
};
use web_sdk::client::{auth::AuthClientExt, chat::ChatClientExt, notebooks::NotebookClientExt};

#[test]
fn sdk_builds_against_contracts_only() {
    let _ = std::any::type_name::<ChatRequest>();
    let _ = std::any::type_name::<ChatResponse>();
    let _ = std::any::type_name::<ChatEvent>();
    let _ = std::any::type_name::<CreateNotebookRequest>();
    let _ = std::any::type_name::<NotebookResponse>();
    let _ = std::any::type_name::<CreateDocumentRequest>();
    let _ = std::any::type_name::<DocumentsResponse>();
    let _ = std::any::type_name::<AuthEnvelope>();
    let _ = std::any::type_name::<RegisterRequest>();
    let _ = std::any::type_name::<LoginRequest>();
    let _ = std::any::type_name::<ShareTokenResponse>();
    let _ = std::any::type_name::<UsageLimitResponse>();
    let _ = std::any::type_name::<UsageResponse>();
    let _ = std::any::type_name::<HealthResponse>();

    fn uses_clients<T: AuthClientExt + ChatClientExt + NotebookClientExt>() {}
    uses_clients::<web_sdk::ApiClient>();
}
