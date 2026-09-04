use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::{ChatEvent, ChatRequest};

pub struct FixtureTransport {
    events: Vec<ChatEvent>,
}

impl FixtureTransport {
    pub fn from_events(events: Vec<ChatEvent>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> &[ChatEvent] {
        &self.events
    }

    pub fn from_json_lines(json_str: &str) -> Result<Self, serde_json::Error> {
        let mut events = Vec::new();
        for line in json_str.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let data = if let Some(stripped) = line.strip_prefix("data:") {
                stripped.trim()
            } else {
                line
            };
            if data == "[DONE]" {
                break;
            }
            let event: ChatEvent = serde_json::from_str(data)?;
            events.push(event);
        }
        Ok(Self { events })
    }
}

impl ChatTransport for FixtureTransport {
    async fn stream_chat(
        &self,
        _request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        let events = self.events.clone();
        let s = async_stream::stream! {
            for event in events {
                if cancellation.is_cancelled() {
                    yield Err(TransportError::Cancelled);
                    return;
                }
                yield Ok(event);
            }
        };
        Ok(Box::pin(s))
    }
}
