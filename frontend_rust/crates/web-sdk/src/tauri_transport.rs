use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::{ChatEvent, ChatRequest};

pub struct TauriIpcTransport {
    /// Phase 0 仅保留确定性 fixture seam；真实 IPC adapter 尚未实现。
    mock_events: Option<Vec<ChatEvent>>,
}

impl TauriIpcTransport {
    pub fn new() -> Self {
        Self { mock_events: None }
    }

    pub fn with_mock_events(events: Vec<ChatEvent>) -> Self {
        Self {
            mock_events: Some(events),
        }
    }
}

impl Default for TauriIpcTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatTransport for TauriIpcTransport {
    async fn stream_chat(
        &self,
        _request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::Cancelled);
        }

        // 如果配置了 mock 序列，直接推流（用于测试与确定性回归）
        if let Some(events) = &self.mock_events {
            let events = events.clone();
            let cancel = cancellation.clone();
            let s = async_stream::stream! {
                for event in events {
                    if cancel.is_cancelled() {
                        yield Err(TransportError::Cancelled);
                        return;
                    }
                    yield Ok(event);
                }
            };
            return Ok(Box::pin(s));
        }

        Err(TransportError::Unavailable(
            "Tauri IPC adapter is not implemented in this Phase 0 skeleton".to_string(),
        ))
    }
}
