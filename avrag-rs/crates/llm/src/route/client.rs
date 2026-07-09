use crate::protocols::{
    AnthropicMessagesProtocol, GeminiProtocol, OpenAiChatProtocol, Protocol,
};
use crate::route::{auth::Auth, endpoint::Endpoint, framing::SseFramer, transport};
use crate::schema::{LlmError, LlmEvent, LlmRequest, LlmResponse};
use async_stream::try_stream;
use futures::Stream;
use reqwest::header::HeaderMap;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct Route<P: Protocol> {
    pub id: String,
    pub provider: String,
    pub protocol: P,
    pub endpoint: Endpoint,
    pub auth: Auth,
    pub framing: super::framing::Framing,
    pub http_client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub enum AnyRoute {
    OpenAi(Route<OpenAiChatProtocol>),
    Anthropic(Route<AnthropicMessagesProtocol>),
    Gemini(Route<GeminiProtocol>),
}

impl AnyRoute {
    pub fn protocol_id(&self) -> &'static str {
        match self {
            Self::OpenAi(route) => route.protocol.protocol_id(),
            Self::Anthropic(route) => route.protocol.protocol_id(),
            Self::Gemini(route) => route.protocol.protocol_id(),
        }
    }

    pub fn provider(&self) -> &str {
        match self {
            Self::OpenAi(route) => &route.provider,
            Self::Anthropic(route) => &route.provider,
            Self::Gemini(route) => &route.provider,
        }
    }

    pub async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        match self {
            Self::OpenAi(route) => route.generate(request).await,
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
    http_client: reqwest::Client,
) -> AnyRoute {
    let provider = config.provider_name();
    match detect_protocol(&config.base_url) {
        DetectedProtocol::AnthropicMessages => AnyRoute::Anthropic(Route {
            id: provider.clone(),
            provider,
            protocol: AnthropicMessagesProtocol,
            endpoint: Endpoint::new(config.base_url.clone(), "/messages"),
            auth: Auth::Anthropic(config.api_key.clone()),
            framing: super::framing::Framing::Sse,
            http_client,
        }),
        DetectedProtocol::Gemini => AnyRoute::Gemini(Route {
            id: provider.clone(),
            provider,
            protocol: GeminiProtocol,
            endpoint: Endpoint::new(config.base_url.clone(), "/models"),
            auth: Auth::XGoogApiKey(config.api_key.clone()),
            framing: super::framing::Framing::Sse,
            http_client,
        }),
        DetectedProtocol::OpenAiChat => AnyRoute::OpenAi(build_openai_chat_route(config, http_client)),
    }
}

#[derive(Debug, Clone, Default)]
pub struct RoutePatch {
    pub id: Option<String>,
    pub provider: Option<String>,
    pub endpoint: Option<Endpoint>,
    pub auth: Option<Auth>,
    pub framing: Option<super::framing::Framing>,
}

impl<P: Protocol> Route<P> {
    pub fn with(mut self, patch: RoutePatch) -> Self {
        if let Some(id) = patch.id {
            self.id = id;
        }
        if let Some(provider) = patch.provider {
            self.provider = provider;
        }
        if let Some(endpoint) = patch.endpoint {
            self.endpoint = self.endpoint.merge(&endpoint);
        }
        if let Some(auth) = patch.auth {
            self.auth = auth;
        }
        if let Some(framing) = patch.framing {
            self.framing = framing;
        }
        self
    }

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

        let response = transport::post_json(&self.http_client, &url, headers, &body, false).await?;
        let response = transport::ensure_success(response, false).await?;
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::parse(format!("failed to read completion JSON: {e}")))?;

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

            let response = transport::post_json(&self.http_client, &url, headers, &body, true).await?;
            let response = transport::ensure_success(response, true).await?;

            let mut framer = SseFramer::new();
            let mut state = self.protocol.initial_state(&req);
            let mut response = response;

            while let Some(chunk) = response.chunk().await.map_err(LlmError::Http)? {
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
    http_client: reqwest::Client,
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
        framing: super::framing::Framing::Sse,
        http_client,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_route_from_config, detect_protocol, DetectedProtocol};

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
    fn build_route_from_config_selects_protocol() {
        let client = reqwest::Client::new();
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
            client.clone(),
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
            client.clone(),
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
            client,
        );
        assert_eq!(openai.protocol_id(), "openai_chat");
    }
}
