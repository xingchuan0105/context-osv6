use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error {status}: {body}")]
    Api { status: u16, body: String },

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Request cancelled")]
    Cancelled,

    #[error("Stream ended without content")]
    EmptyStream,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl LlmError {
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse(message.into())
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn retry_after(&self) -> Option<Duration> {
        let _ = self;
        None
    }
}
