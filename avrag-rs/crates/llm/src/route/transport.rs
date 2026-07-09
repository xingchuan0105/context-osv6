use crate::schema::LlmError;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Serialize;

pub async fn post_json(
    client: &reqwest::Client,
    url: &str,
    headers: HeaderMap,
    body: &impl Serialize,
    stream: bool,
) -> Result<reqwest::Response, LlmError> {
    let mut request = client.post(url).headers(headers);
    request = request.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if stream {
        request = request.header(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        );
    }
    Ok(request.json(body).send().await?)
}

pub async fn ensure_success(
    response: reqwest::Response,
    stream: bool,
) -> Result<reqwest::Response, LlmError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let context = if stream {
        "Chat completion stream API error"
    } else {
        "Chat completion API error"
    };
    Err(LlmError::Api {
        status,
        body: format!("{context} {status}: {body}"),
    })
}
