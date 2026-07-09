use super::Provider;
use crate::protocols::OpenAiChatProtocol;
use crate::route::{Auth, Endpoint, Framing, Route};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub fn configure(api_key: String, base_url: Option<String>) -> Provider {
    let base = base_url
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let auth = if api_key.is_empty() {
        Auth::None
    } else {
        Auth::Bearer(api_key)
    };
    let route = Route {
        id: "openai".to_string(),
        provider: "openai".to_string(),
        protocol: OpenAiChatProtocol,
        endpoint: Endpoint::new(base, "/chat/completions"),
        auth,
        framing: Framing::Sse,
        http_client: default_http_client(),
    };
    Provider::from_route("openai", route)
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("reqwest client should build")
}
