use super::Provider;
use crate::protocols::AnthropicMessagesProtocol;
use crate::route::{Auth, Endpoint, Framing, Route};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";

pub fn configure(api_key: String, base_url: Option<String>) -> Provider {
    let base = base_url
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let route = Route {
        id: "anthropic".to_string(),
        provider: "anthropic".to_string(),
        protocol: AnthropicMessagesProtocol,
        endpoint: Endpoint::new(base, "/messages"),
        auth: Auth::Anthropic(api_key),
        framing: Framing::Sse,
        http_client: default_http_client(),
    };
    Provider::from_route("anthropic", route)
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("reqwest client should build")
}
