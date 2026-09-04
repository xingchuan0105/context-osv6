pub mod browser_rest;
pub mod browser_transport;
pub mod client;
pub mod conversation_api;
pub mod fixture_transport;
pub mod sse_decoder;
pub mod tauri_transport;
pub mod transport;

pub use browser_rest::BrowserRestClient;
pub use browser_transport::BrowserHttpTransport;
pub use client::ChatClient;
pub use conversation_api::{
    encode_path_segment, parse_message_list, parse_session, parse_session_list, session_messages_url,
    session_url, sessions_url, trim_base_url,
};
pub use fixture_transport::FixtureTransport;
pub use sse_decoder::{SseDecoder, events_from_byte_stream};
pub use tauri_transport::{
    TauriIpcTransport, is_tauri_runtime, map_ipc_error_status, parse_ipc_event, prepare_ipc_request,
};
pub use transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
