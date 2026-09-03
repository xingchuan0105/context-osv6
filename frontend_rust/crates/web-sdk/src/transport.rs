use contracts::chat::{ChatEvent, ChatRequest};
use futures_util::Stream;
use std::pin::Pin;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Network connection error: {0}")]
    Network(String),
    #[error("Authentication required (HTTP 401)")]
    Unauthorized,
    #[error("Forbidden (HTTP 403): {0}")]
    Forbidden(String),
    #[error("Payment required / Quota exceeded (HTTP 402): {0}")]
    PaymentRequired(String),
    #[error("Rate limited (HTTP 429)")]
    RateLimited,
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("Stream heartbeat timeout")]
    HeartbeatTimeout,
    #[error("Invalid event framing: {0}")]
    Framing(String),
    #[error("Failed to parse event JSON: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Request cancelled by client")]
    Cancelled,
    #[error("Transport unavailable: {0}")]
    Unavailable(String),
}

#[derive(Clone, Default)]
pub struct Cancellation {
    token: CancellationToken,
}

impl Cancellation {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

pub type ChatEventStream = Pin<Box<dyn Stream<Item = Result<ChatEvent, TransportError>> + Send>>;

pub trait ChatTransport: Send + Sync {
    fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> impl std::future::Future<Output = Result<ChatEventStream, TransportError>> + Send;
}
