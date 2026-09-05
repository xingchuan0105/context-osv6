pub mod auth;
pub mod billing_api;
pub mod browser_auth;
pub mod browser_rest;
pub mod browser_transport;
pub mod client;
pub mod conversation_api;
pub mod fixture_transport;
pub mod citations;
pub mod markdown;
pub mod progress;
pub mod providers;
pub mod scope;
pub mod session_files;
pub mod share_api;
pub mod reducer;
pub mod sse_decoder;
pub mod transport;
pub mod workspace_api;

pub use auth::{
    AUTH_BOOTSTRAP_TIMEOUT_MS, AUTH_PERSISTED_COOKIE_NAME, AUTH_SESSION_COOKIE_MAX_AGE,
    AUTH_SESSION_COOKIE_NAME, AUTH_SESSION_COOKIE_VALUE, AUTH_STORAGE_KEY, AuthUser, PersistedAuth,
    auth_me_url, cookie_security_attrs, decode_uri_component, encode_uri_component,
    has_auth_session_hint, parse_auth_me_json, parse_cookie_value, parse_persisted_auth_cookie,
    parse_persisted_auth_json, persisted_clear_cookie, persisted_set_cookie,
    session_hint_clear_cookie, session_hint_set_cookie,
};
pub use billing_api::{
    BillingOrderStatusResponse, BillingPlan, BillingPlansResponse, CheckoutRequest,
    CheckoutResponse, TopupPack, WalletBalanceResponse, billing_plans_url, checkout_session_url,
    order_status_url, parse_billing_plans, parse_checkout_response, parse_order_status,
    parse_topup_packs, parse_wallet_balance, topup_packs_url, wallet_balance_url,
};
pub use browser_auth::{
    auth_login, auth_register, auth_reset_confirm, auth_reset_send_code, auth_reset_verify_code,
    clear_browser_auth, fetch_me, read_browser_auth, write_browser_auth,
};
pub use browser_rest::BrowserRestClient;
pub use browser_transport::BrowserHttpTransport;
pub use client::ChatClient;
pub use conversation_api::{
    encode_path_segment, message_feedback_url, parse_message_list, parse_session, parse_session_list,
    session_messages_url, session_url, sessions_url, trim_base_url,
};
pub use fixture_transport::FixtureTransport;
pub use citations::{CitationView, RenderedAnswer, SourceCard, render_assistant_answer};
pub use markdown::render_assistant_markdown;
pub use progress::{activities_for_display, progress_folded, progress_summary_label};
pub use providers::{
    ProviderSecretRow, ProviderSecretsResponse, has_quick_chat_byok, model_role_label,
    parse_provider_secrets, provider_secrets_url,
};
pub use scope::{
    Capability, capabilities_to_wire, derive_agent_type_label, mode_line, normalize_capabilities,
    reconcile_session_rag, toggle_capability,
};
pub use session_files::{
    SESSION_FILE_ACCEPT, TrayFileStatus, complete_upload_url, create_session_json, create_upload_json,
    files_block_send, parse_session_files, parse_upload_response, ready_file_count,
    reindex_document_url, resolve_upload_url, session_file_url, session_files_url, tray_status,
    tray_status_attr, tray_status_label,
};
pub use reducer::{
    ActivityEntry, ChatTurnState, TurnStatus, event_request_id, reduce_chat_event,
};
pub use sse_decoder::{SseDecoder, events_from_byte_stream};
pub use share_api::{
    parse_access_logs, parse_share_analytics, parse_share_settings, parse_share_token,
    parse_shared_workspace, public_user_shares_url, share_access_logs_url, share_analytics_url,
    share_settings_url, share_url, shared_kb_url,
};
pub use transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
pub use workspace_api::{
    create_note_json, create_workspace_json, parse_workspace_documents, parse_workspace_list,
    parse_workspace_notes, parse_workspace_response, workspace_document_url,
    workspace_documents_url, workspace_note_url, workspace_notes_url, workspace_url,
    workspaces_url,
};
