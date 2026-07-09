use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Auth {
    None,
    Bearer(String),
    XApiKey(String),
    XGoogApiKey(String),
    Anthropic(String),
    Custom(String, String),
}

impl Auth {
    pub fn apply(&self, headers: &mut HeaderMap) {
        match self {
            Self::None => {}
            Self::Bearer(key) => {
                if let Ok(value) = HeaderValue::from_str(&format!("Bearer {key}")) {
                    headers.insert(reqwest::header::AUTHORIZATION, value);
                }
            }
            Self::XApiKey(key) => insert_header(headers, "x-api-key", key),
            Self::XGoogApiKey(key) => insert_header(headers, "x-goog-api-key", key),
            Self::Anthropic(key) => {
                insert_header(headers, "x-api-key", key);
                insert_header(headers, "anthropic-version", "2023-06-01");
            }
            Self::Custom(name, value) => insert_header(headers, name, value),
        }
    }

    pub fn or_else(self, fallback: Self) -> Self {
        match self {
            Self::None => fallback,
            other => other,
        }
    }
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_str(name),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

#[cfg(test)]
mod tests {
    use super::Auth;

    #[test]
    fn bearer_auth_sets_authorization_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        Auth::Bearer("secret".to_string()).apply(&mut headers);
        assert_eq!(
            headers
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok()),
            Some("Bearer secret")
        );
    }
}
