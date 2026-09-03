use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::ChatRequest;

pub struct BrowserHttpTransport {
    pub base_url: String,
    pub auth_token: Option<String>,
}

impl BrowserHttpTransport {
    pub fn new(base_url: &str, auth_token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/api/v1/chat", self.base_url)
    }

    pub fn build_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Content-Type", "application/json".to_string()),
            ("Accept", "text/event-stream".to_string()),
        ];
        if let Some(token) = &self.auth_token {
            headers.push(("Authorization", format!("Bearer {}", token)));
        }
        headers
    }
}

impl ChatTransport for BrowserHttpTransport {
    async fn stream_chat(
        &self,
        _request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        // 在非浏览器环境（如集成测试中直接调用），若未配置真实端点则返回清晰错误
        if cancellation.is_cancelled() {
            return Err(TransportError::Cancelled);
        }
        if self.base_url.is_empty() {
            return Err(TransportError::Network(
                "Base URL cannot be empty".to_string(),
            ));
        }

        Err(TransportError::Unavailable(
            "browser Fetch/SSE adapter is not implemented in this Phase 0 skeleton".to_string(),
        ))
    }
}
