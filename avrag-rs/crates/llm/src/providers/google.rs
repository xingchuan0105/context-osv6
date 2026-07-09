use super::Provider;
use crate::protocols::GeminiProtocol;
use crate::route::{Auth, Endpoint, Framing, Route};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub fn configure(api_key: String, base_url: Option<String>) -> Provider {
    let base = base_url
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let route = Route {
        id: "google".to_string(),
        provider: "google".to_string(),
        protocol: GeminiProtocol,
        endpoint: Endpoint::new(base, "/models"),
        auth: Auth::XGoogApiKey(api_key),
        framing: Framing::Sse,
        http_client: default_http_client(),
    };
    Provider::from_route("google", route)
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("reqwest client should build")
}
