use crate::schema::LlmError;
use futures::Stream;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use std::pin::Pin;

/// A transport for issuing LLM HTTP requests. The production implementation is
/// [`ReqwestTransport`]; tests inject a fake so the pool path runs offline.
#[async_trait::async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug + 'static {
    async fn post_json(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &Value,
        stream: bool,
    ) -> Result<TransportBody, LlmError>;
}

/// Response of a [`Transport::post_json`] call. Non-streaming responses are
/// buffered and status-checked into a parsed JSON value; streaming responses
/// are returned as an SSE byte chunk stream.
pub enum TransportBody {
    Json(Value),
    Chunks(Pin<Box<dyn Stream<Item = Result<Vec<u8>, LlmError>> + Send>>),
}

/// Production transport backed by a reqwest client.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Transport for ReqwestTransport {
    async fn post_json(
        &self,
        url: &str,
        headers: HeaderMap,
        body: &Value,
        stream: bool,
    ) -> Result<TransportBody, LlmError> {
        let mut request = self.client.post(url).headers(headers);
        request = request.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if stream {
            request = request.header(
                reqwest::header::ACCEPT,
                HeaderValue::from_static("text/event-stream"),
            );
        }
        let response = request.json(body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let context = if stream {
                "Chat completion stream API error"
            } else {
                "Chat completion API error"
            };
            return Err(LlmError::Api {
                status,
                body: format!("{context} {status}: {body}"),
            });
        }

        if stream {
            Ok(TransportBody::Chunks(Box::pin(stream_chunks(response))))
        } else {
            let value = response
                .json()
                .await
                .map_err(|e| LlmError::parse(format!("failed to read completion JSON: {e}")))?;
            Ok(TransportBody::Json(value))
        }
    }
}

fn stream_chunks(
    mut response: reqwest::Response,
) -> impl Stream<Item = Result<Vec<u8>, LlmError>> + Send {
    async_stream::stream! {
        while let Some(chunk) = response.chunk().await.map_err(LlmError::Http)? {
            yield Ok(chunk.to_vec());
        }
    }
}
