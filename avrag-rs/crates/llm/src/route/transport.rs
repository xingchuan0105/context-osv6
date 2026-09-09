use crate::schema::LlmError;
use futures::Stream;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use std::pin::Pin;

/// A completion may outlive its timeout while tokens keep arriving. Bound
/// connection establishment and each idle read, not the whole generation.
pub(crate) fn completion_http_client(timeout_ms: u64) -> reqwest::Client {
    let timeout = std::time::Duration::from_millis(timeout_ms);
    reqwest::Client::builder()
        .connect_timeout(timeout)
        .read_timeout(timeout)
        .build()
        .expect("reqwest client should build")
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::post};
    use futures::StreamExt;
    use std::{convert::Infallible, time::Duration};

    async fn server() -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route(
                "/active",
                post(|| async {
                    Body::from_stream(async_stream::stream! {
                        for _ in 0..8 {
                            tokio::time::sleep(Duration::from_millis(75)).await;
                            yield Ok::<_, Infallible>("data: token\n\n");
                        }
                    })
                }),
            )
            .route(
                "/idle",
                post(|| async {
                    Body::from_stream(async_stream::stream! {
                        yield Ok::<_, Infallible>("data: first\n\n");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        yield Ok::<_, Infallible>("data: late\n\n");
                    })
                }),
            )
            .route(
                "/headers",
                post(|| async {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    "{}"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, handle)
    }

    #[tokio::test]
    async fn active_completion_stream_outlives_read_timeout() {
        let (url, server) = server().await;
        let transport = ReqwestTransport::new(completion_http_client(250));
        let body = transport
            .post_json(
                &format!("{url}/active"),
                HeaderMap::new(),
                &Value::Null,
                true,
            )
            .await
            .unwrap();
        let TransportBody::Chunks(mut chunks) = body else {
            panic!("stream expected")
        };
        let mut bytes = Vec::new();
        while let Some(chunk) = chunks.next().await {
            bytes.extend(chunk.unwrap());
        }
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "data: token\n\n".repeat(8)
        );
        server.abort();
    }

    #[tokio::test]
    async fn stalled_completion_stream_still_times_out() {
        let (url, server) = server().await;
        let transport = ReqwestTransport::new(completion_http_client(250));
        let body = transport
            .post_json(&format!("{url}/idle"), HeaderMap::new(), &Value::Null, true)
            .await
            .unwrap();
        let TransportBody::Chunks(mut chunks) = body else {
            panic!("stream expected")
        };
        assert!(chunks.next().await.unwrap().is_ok());
        assert!(matches!(chunks.next().await.unwrap(), Err(LlmError::Http(e)) if e.is_timeout()));
        server.abort();
    }

    #[tokio::test]
    async fn completion_waiting_for_headers_still_times_out() {
        let (url, server) = server().await;
        let transport = ReqwestTransport::new(completion_http_client(250));
        for stream in [false, true] {
            let result = transport
                .post_json(
                    &format!("{url}/headers"),
                    HeaderMap::new(),
                    &Value::Null,
                    stream,
                )
                .await;
            assert!(matches!(result, Err(LlmError::Http(e)) if e.is_timeout()));
        }
        server.abort();
    }
}
