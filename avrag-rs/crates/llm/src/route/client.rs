use crate::protocols::{
    AnthropicMessagesProtocol, GeminiProtocol, OpenAiChatProtocol, OpenAiResponsesProtocol,
    Protocol,
};
use crate::route::{auth::Auth, endpoint::Endpoint, framing::SseFramer, Transport, TransportBody};
use crate::schema::{LlmError, LlmEvent, LlmRequest, LlmResponse};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::header::HeaderMap;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Route<P: Protocol> {
    pub id: String,
    pub provider: String,
    pub protocol: P,
    pub endpoint: Endpoint,
    pub auth: Auth,
    pub transport: Arc<dyn Transport>,
}

#[derive(Debug, Clone)]
pub enum AnyRoute {
    OpenAi(Route<OpenAiChatProtocol>),
    OpenAiResponses(Route<OpenAiResponsesProtocol>),
    Anthropic(Route<AnthropicMessagesProtocol>),
    Gemini(Route<GeminiProtocol>),
}

impl AnyRoute {
    pub fn protocol_id(&self) -> &'static str {
        match self {
            Self::OpenAi(route) => route.protocol.protocol_id(),
            Self::OpenAiResponses(route) => route.protocol.protocol_id(),
            Self::Anthropic(route) => route.protocol.protocol_id(),
            Self::Gemini(route) => route.protocol.protocol_id(),
        }
    }

    pub fn provider(&self) -> &str {
        match self {
            Self::OpenAi(route) => &route.provider,
            Self::OpenAiResponses(route) => &route.provider,
            Self::Anthropic(route) => &route.provider,
            Self::Gemini(route) => &route.provider,
        }
    }

    pub async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        match self {
            Self::OpenAi(route) => route.generate(request).await,
            Self::OpenAiResponses(route) => route.generate(request).await,
            Self::Anthropic(route) => route.generate(request).await,
            Self::Gemini(route) => route.generate(request).await,
        }
    }

    pub fn stream<'a>(
        &'a self,
        request: LlmRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<LlmEvent, LlmError>> + Send + 'a>> {
        match self {
            Self::OpenAi(route) => route.stream(request),
            Self::OpenAiResponses(route) => route.stream(request),
            Self::Anthropic(route) => route.stream(request),
            Self::Gemini(route) => route.stream(request),
        }
    }

    pub fn is_openai_chat(&self) -> bool {
        matches!(self, Self::OpenAi(_))
    }

    pub fn openai_route(&self) -> Option<&Route<OpenAiChatProtocol>> {
        match self {
            Self::OpenAi(route) => Some(route),
            _ => None,
        }
    }
}

impl From<Route<OpenAiChatProtocol>> for AnyRoute {
    fn from(route: Route<OpenAiChatProtocol>) -> Self {
        Self::OpenAi(route)
    }
}

impl From<Route<OpenAiResponsesProtocol>> for AnyRoute {
    fn from(route: Route<OpenAiResponsesProtocol>) -> Self {
        Self::OpenAiResponses(route)
    }
}

impl From<Route<AnthropicMessagesProtocol>> for AnyRoute {
    fn from(route: Route<AnthropicMessagesProtocol>) -> Self {
        Self::Anthropic(route)
    }
}

impl From<Route<GeminiProtocol>> for AnyRoute {
    fn from(route: Route<GeminiProtocol>) -> Self {
        Self::Gemini(route)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
    Gemini,
}

pub fn detect_protocol(base_url: &str) -> DetectedProtocol {
    let url = base_url.to_ascii_lowercase();
    if url.contains("anthropic.com") {
        DetectedProtocol::AnthropicMessages
    } else if url.contains("googleapis.com") && !url.contains("/openai") {
        DetectedProtocol::Gemini
    } else {
        DetectedProtocol::OpenAiChat
    }
}

pub fn build_route_from_config(
    config: &crate::ModelProviderConfig,
    transport: Arc<dyn Transport>,
) -> AnyRoute {
    // Explicit `api_style=responses` opts a provider into the Responses
    // protocol even though its base_url is shared with chat completions
    // (e.g. `https://api.deepseek.com` serves both).
    if config.api_style == Some(crate::ApiStyle::OpenAiResponses) {
        return AnyRoute::OpenAiResponses(build_openai_responses_route(config, transport));
    }

    let provider = config.provider_name();
    match detect_protocol(&config.base_url) {
        DetectedProtocol::AnthropicMessages => AnyRoute::Anthropic(Route {
            id: provider.clone(),
            provider,
            protocol: AnthropicMessagesProtocol,
            endpoint: Endpoint::new(config.base_url.clone(), "/messages"),
            auth: Auth::Anthropic(config.api_key.clone()),
            transport,
        }),
        DetectedProtocol::Gemini => AnyRoute::Gemini(Route {
            id: provider.clone(),
            provider,
            protocol: GeminiProtocol,
            endpoint: Endpoint::new(config.base_url.clone(), "/models"),
            auth: Auth::XGoogApiKey(config.api_key.clone()),
            transport,
        }),
        DetectedProtocol::OpenAiChat | DetectedProtocol::OpenAiResponses => {
            AnyRoute::OpenAi(build_openai_chat_route(config, transport))
        }
    }
}

impl<P: Protocol> Route<P> {
    fn render_url(&self, req: &LlmRequest) -> Result<String, LlmError> {
        let mut endpoint = self.endpoint.clone();
        if let Some(path) = self.protocol.endpoint_path(req) {
            endpoint.path = path;
        }
        let extra_query = self.protocol.endpoint_query(req);
        if !extra_query.is_empty() {
            endpoint.query.extend(extra_query);
        }
        endpoint.render()
    }

    pub async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut req = request;
        req.options.stream = false;

        let body = self.protocol.build_body(&req)?;
        let url = self.render_url(&req)?;
        let mut headers = HeaderMap::new();
        self.auth.apply(&mut headers);
        let body_value = serde_json::to_value(&body)
            .map_err(|e| LlmError::parse(format!("failed to serialize completion body: {e}")))?;

        let value = match self
            .transport
            .post_json(&url, headers, &body_value, false)
            .await?
        {
            TransportBody::Json(value) => value,
            TransportBody::Chunks(_) => {
                return Err(LlmError::protocol(
                    "unexpected streaming response for non-streaming request",
                ))
            }
        };

        let mut state = self.protocol.initial_state(&req);
        let _events = self.protocol.step(&mut state, &value)?;
        for event in self.protocol.on_halt(&state) {
            if let LlmEvent::ProviderError { message, .. } = event {
                return Err(LlmError::protocol(message));
            }
        }
        self.protocol.finalize(state)
    }

    pub fn stream<'a>(
        &'a self,
        request: LlmRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<LlmEvent, LlmError>> + Send + 'a>> {
        let stream = try_stream! {
            let mut req = request;
            req.options.stream = true;
            let body = self.protocol.build_body(&req)?;
            let url = self.render_url(&req)?;
            let mut headers = HeaderMap::new();
            self.auth.apply(&mut headers);
            let body_value = serde_json::to_value(&body)
                .map_err(|e| LlmError::parse(format!("failed to serialize completion body: {e}")))?;

            let TransportBody::Chunks(mut chunks) = self
                .transport
                .post_json(&url, headers, &body_value, true)
                .await?
            else {
                panic!("transport must not return a buffered body for a streaming request")
            };

            let mut framer = SseFramer::new();
            let mut state = self.protocol.initial_state(&req);

            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                let frames = framer.feed_chunk(&chunk)?;
                for frame in frames {
                    let event = self.protocol.decode_frame(&frame)?;
                    for llm_event in self.protocol.step(&mut state, &event)? {
                        yield llm_event;
                    }
                }
            }

            for frame in framer.finish()? {
                let event = self.protocol.decode_frame(&frame)?;
                for llm_event in self.protocol.step(&mut state, &event)? {
                    yield llm_event;
                }
            }

            for llm_event in self.protocol.on_halt(&state) {
                yield llm_event;
            }
        };
        Box::pin(stream)
    }
}

pub fn build_openai_chat_route(
    config: &crate::ModelProviderConfig,
    transport: Arc<dyn Transport>,
) -> Route<OpenAiChatProtocol> {
    let auth = if config.api_key.is_empty()
        && config.base_url.to_ascii_lowercase().contains("localhost")
    {
        Auth::None
    } else if config.api_key.is_empty() {
        Auth::None
    } else {
        Auth::Bearer(config.api_key.clone())
    };
    Route {
        id: config.provider_name(),
        provider: config.provider_name(),
        protocol: OpenAiChatProtocol,
        endpoint: Endpoint::new(config.base_url.clone(), "/chat/completions"),
        auth,
        transport,
    }
}

pub fn build_openai_responses_route(
    config: &crate::ModelProviderConfig,
    transport: Arc<dyn Transport>,
) -> Route<OpenAiResponsesProtocol> {
    let auth = if config.api_key.is_empty()
        && config.base_url.to_ascii_lowercase().contains("localhost")
    {
        Auth::None
    } else if config.api_key.is_empty() {
        Auth::None
    } else {
        Auth::Bearer(config.api_key.clone())
    };
    Route {
        id: config.provider_name(),
        provider: config.provider_name(),
        protocol: OpenAiResponsesProtocol,
        // OpenAI SDK appends `/v1/responses` for base URLs without a version
        // segment (e.g. `https://api.deepseek.com`); keep the same semantics.
        endpoint: Endpoint::new(config.base_url.clone(), "/v1/responses"),
        auth,
        transport,
    }
}

#[cfg(test)]
mod tests {
    use super::{AnyRoute, DetectedProtocol, build_route_from_config, detect_protocol};
    use crate::route::ReqwestTransport;
    use std::sync::Arc;

    fn offline_transport() -> Arc<dyn crate::route::Transport> {
        Arc::new(ReqwestTransport::new(reqwest::Client::new()))
    }

    #[test]
    fn detect_protocol_routes_anthropic_and_gemini() {
        assert_eq!(
            detect_protocol("https://api.anthropic.com/v1"),
            DetectedProtocol::AnthropicMessages
        );
        assert_eq!(
            detect_protocol("https://generativelanguage.googleapis.com/v1beta"),
            DetectedProtocol::Gemini
        );
        assert_eq!(
            detect_protocol("https://generativelanguage.googleapis.com/v1beta/openai"),
            DetectedProtocol::OpenAiChat
        );
        assert_eq!(
            detect_protocol("https://api.deepseek.com"),
            DetectedProtocol::OpenAiChat
        );
    }

    #[test]
    fn api_style_responses_selects_responses_protocol() {
        let transport = offline_transport();
        let responses = build_route_from_config(
            &crate::ModelProviderConfig {
                base_url: "https://api.deepseek.com".into(),
                api_key: "k".into(),
                model: "deepseek-v4-flash".into(),
                timeout_ms: 1000,
                api_style: Some(crate::ApiStyle::OpenAiResponses),
                dimensions: None,
                enable_thinking: Some(true),
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            transport.clone(),
        );
        assert_eq!(responses.protocol_id(), "openai_responses");
        let endpoint = match &responses {
            AnyRoute::OpenAiResponses(route) => route.endpoint.render().unwrap(),
            _ => panic!("expected responses route"),
        };
        assert_eq!(endpoint, "https://api.deepseek.com/v1/responses");

        // Explicit `openai` style still selects chat completions.
        let chat = build_route_from_config(
            &crate::ModelProviderConfig {
                base_url: "https://api.deepseek.com".into(),
                api_key: "k".into(),
                model: "deepseek-v4-flash".into(),
                timeout_ms: 1000,
                api_style: Some(crate::ApiStyle::OpenAi),
                dimensions: None,
                enable_thinking: Some(true),
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            transport,
        );
        assert_eq!(chat.protocol_id(), "openai_chat");
    }

    #[test]
    fn build_route_from_config_selects_protocol() {
        let transport = offline_transport();
        let anthropic = build_route_from_config(
            &crate::ModelProviderConfig {
                base_url: "https://api.anthropic.com/v1".into(),
                api_key: "k".into(),
                model: "claude".into(),
                timeout_ms: 1000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            transport.clone(),
        );
        assert_eq!(anthropic.protocol_id(), "anthropic_messages");

        let gemini = build_route_from_config(
            &crate::ModelProviderConfig {
                base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
                api_key: "k".into(),
                model: "gemini-2.0-flash".into(),
                timeout_ms: 1000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            transport.clone(),
        );
        assert_eq!(gemini.protocol_id(), "gemini");

        let openai = build_route_from_config(
            &crate::ModelProviderConfig {
                base_url: "https://api.openai.com/v1".into(),
                api_key: "k".into(),
                model: "gpt-4o".into(),
                timeout_ms: 1000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            transport,
        );
        assert_eq!(openai.protocol_id(), "openai_chat");
    }
}
