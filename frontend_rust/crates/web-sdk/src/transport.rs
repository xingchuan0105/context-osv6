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
    #[error("Response body is empty")]
    EmptyBody,
    #[error("Stream interrupted mid-way: {0}")]
    Interrupted(String),
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

#[cfg(target_arch = "wasm32")]
pub(crate) const MAX_ERROR_BODY_BYTES: usize = 4096;

impl TransportError {
    pub fn from_http_status(status: u16, body: String) -> Self {
        match status {
            401 => Self::Unauthorized,
            402 => Self::PaymentRequired(body),
            403 => Self::Forbidden(body),
            429 => Self::RateLimited,
            _ => Self::HttpStatus { status, body },
        }
    }
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

    /// 等待取消信号（协作式；浏览器路径用它驱动底层 Fetch abort）
    pub async fn cancelled(&self) {
        self.token.cancelled().await;
    }
}

/// 事件流类型按目标平台收窄：浏览器 Fetch/ReadableStream 对象是 !Send，
/// wasm32 下不伪装线程安全（Gate C：不要用无意义 wrapper 假装 Send）。
#[cfg(not(target_arch = "wasm32"))]
pub type ChatEventStream = Pin<Box<dyn Stream<Item = Result<ChatEvent, TransportError>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type ChatEventStream = Pin<Box<dyn Stream<Item = Result<ChatEvent, TransportError>>>>;

#[cfg(not(target_arch = "wasm32"))]
pub trait ChatTransport: Send + Sync {
    fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> impl std::future::Future<Output = Result<ChatEventStream, TransportError>> + Send;
}

#[cfg(target_arch = "wasm32")]
pub trait ChatTransport {
    fn stream_chat(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> impl std::future::Future<Output = Result<ChatEventStream, TransportError>>;
}
