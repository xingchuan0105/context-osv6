use crate::transport::{Cancellation, ChatEventStream, ChatTransport, TransportError};
use contracts::chat::ChatRequest;

pub struct ChatClient<T: ChatTransport> {
    transport: T,
}

impl<T: ChatTransport> ChatClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub async fn stream(
        &self,
        request: ChatRequest,
        cancellation: Cancellation,
    ) -> Result<ChatEventStream, TransportError> {
        self.transport.stream_chat(request, cancellation).await
    }
}
