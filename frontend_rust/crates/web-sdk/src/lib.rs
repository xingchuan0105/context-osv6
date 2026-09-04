pub mod auth;
pub mod browser_auth;
pub mod browser_rest;
pub mod browser_transport;
pub mod client;
pub mod conversation_api;
pub mod fixture_transport;
pub mod reducer;
pub mod sse_decoder;
pub mod transport;

pub use auth::{
    AUTH_BOOTSTRAP_TIMEOUT_MS, AUTH_PERSISTED_COOKIE_NAME, AUTH_SESSION_COOKIE_MAX_AGE,
    AUTH_SESSION_COOKIE_NAME, AUTH_SESSION_COOKIE_VALUE, AUTH_STORAGE_KEY, AuthUser, PersistedAuth,
    auth_me_url, cookie_security_attrs, decode_uri_component, encode_uri_component,
    has_auth_session_hint, parse_auth_me_json, parse_cookie_value, parse_persisted_auth_cookie,
    parse_persisted_auth_json, persisted_clear_cookie, persisted_set_cookie,
    session_hint_clear_cookie, session_hint_set_cookie,
};
pub use browser_auth::{clear_browser_auth, fetch_me, read_browser_auth, write_browser_auth};
pub use browser_rest::BrowserRestClient;
pub use browser_transport::BrowserHttpTransport;
pub use client::ChatClient;
pub use conversation_api::{
    encode_path_segment, parse_message_list, parse_session, parse_session_list, session_messages_url,
    session_url, sessions_url, trim_base_url,
};
pub use fixture_transport::FixtureTransport;
pub use reducer::{
    ActivityEntry, ChatTurnState, TurnStatus, event_request_id, reduce_chat_event,
};
pub use sse_decoder::{SseDecoder, events_from_byte_stream};
pub use transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
