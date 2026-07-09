use crate::schema::{LlmError, LlmEvent, LlmRequest, LlmResponse};
use serde::Serialize;

pub mod anthropic_messages;
pub mod gemini;
pub mod openai_chat;

pub use anthropic_messages::AnthropicMessagesProtocol;
pub use gemini::GeminiProtocol;
pub use openai_chat::OpenAiChatProtocol;

pub trait Protocol: Clone + Send + Sync + 'static {
    type Body: Serialize + Send + Sync;
    type State: Default + Send;

    fn protocol_id(&self) -> &'static str;
    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError>;
    fn initial_state(&self, req: &LlmRequest) -> Self::State;
    fn decode_frame(&self, frame: &str) -> Result<serde_json::Value, LlmError>;
    fn step(
        &self,
        state: &mut Self::State,
        event: &serde_json::Value,
    ) -> Result<Vec<LlmEvent>, LlmError>;

    /// Override the route endpoint path for this request (e.g. Gemini model-in-path URLs).
    fn endpoint_path(&self, req: &LlmRequest) -> Option<String> {
        let _ = req;
        None
    }

    /// Optional query parameters appended to the endpoint URL.
    fn endpoint_query(&self, req: &LlmRequest) -> Vec<(String, String)> {
        let _ = req;
        Vec::new()
    }

    fn on_halt(&self, state: &Self::State) -> Vec<LlmEvent> {
        let _ = state;
        Vec::new()
    }

    fn finalize(&self, state: Self::State) -> Result<LlmResponse, LlmError>;
}
