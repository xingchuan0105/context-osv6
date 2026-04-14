//! SSE (Server-Sent Events) client for chat streaming
//!
//! Parses SSE events from the `/api/v1/chat` endpoint into typed events.

#[cfg(not(target_arch = "wasm32"))]
use async_stream::stream;
use futures_util::Stream;
#[cfg(not(target_arch = "wasm32"))]
use futures_util::StreamExt;
#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;
use std::pin::Pin;

use contracts::chat::{ChatEvent, ChatRequest};

#[cfg(target_arch = "wasm32")]
type BoxedSseStream = Pin<Box<dyn Stream<Item = SseEvent>>>;
#[cfg(not(target_arch = "wasm32"))]
type BoxedSseStream = Pin<Box<dyn Stream<Item = SseEvent> + Send>>;

pub type SseEvent = ChatEvent;

fn parse_sse_event(event_name: &str, data: &str) -> Option<SseEvent> {
    let mut payload: serde_json::Value = serde_json::from_str(data).ok()?;
    if payload.get("event").is_none() {
        let canonical_event = match event_name {
            "start" | "trace" | "token" | "citations" | "done" | "error" => event_name,
            _ => return None,
        };
        payload
            .as_object_mut()?
            .insert("event".to_string(), canonical_event.into());
    }
    serde_json::from_value(payload).ok()
}

#[cfg(target_arch = "wasm32")]
fn parse_sse_events_from_text(text: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if line.is_empty() {
            if !data_lines.is_empty() {
                if let Some(event) = parse_sse_event(
                    if event_name.is_empty() {
                        "message"
                    } else {
                        &event_name
                    },
                    &data_lines.join("\n"),
                ) {
                    events.push(event);
                }
            }
            event_name.clear();
            data_lines.clear();
            continue;
        }

        if let Some(value) = line.strip_prefix("event:") {
            event_name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim().to_string());
        }
    }

    if !data_lines.is_empty() {
        if let Some(event) = parse_sse_event(
            if event_name.is_empty() {
                "message"
            } else {
                &event_name
            },
            &data_lines.join("\n"),
        ) {
            events.push(event);
        }
    }

    events
}

/// Creates a stream of typed SseEvents from an SSE response body.
#[cfg(not(target_arch = "wasm32"))]
pub fn sse_stream(body: reqwest::Response) -> BoxedSseStream {
    let stream = stream! {
        let mut event_name = String::new();
        let mut data_lines: Vec<String> = Vec::new();
        let mut buffer = String::new();
        let mut bytes_stream = body.bytes_stream();

        while let Some(chunk) = bytes_stream.next().await {
            match chunk {
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    while let Some(pos) = buffer.find('\n') {
                        let mut line = buffer[..pos].to_string();
                        buffer.drain(..=pos);

                        if line.ends_with('\r') {
                            line.pop();
                        }

                        if line.is_empty() {
                            if !data_lines.is_empty() {
                                if let Some(event) = parse_sse_event(
                                    if event_name.is_empty() { "message" } else { &event_name },
                                    &data_lines.join("\n"),
                                ) {
                                    yield event;
                                }
                            }
                            event_name.clear();
                            data_lines.clear();
                            continue;
                        }

                        if let Some(value) = line.strip_prefix("event:") {
                            event_name = value.trim().to_string();
                        } else if let Some(value) = line.strip_prefix("data:") {
                            data_lines.push(value.trim().to_string());
                        }
                    }
                }
                Err(error) => {
                    yield SseEvent::Error {
                        request_id: String::new(),
                        code: "client_stream_error".to_string(),
                        message: error.to_string(),
                    };
                    return;
                }
            }
        }

        if !data_lines.is_empty() {
            if let Some(event) = parse_sse_event(
                if event_name.is_empty() { "message" } else { &event_name },
                &data_lines.join("\n"),
            ) {
                yield event;
            }
        }
    };

    Box::pin(stream)
}

#[cfg(target_arch = "wasm32")]
pub fn sse_stream(events: Vec<SseEvent>) -> BoxedSseStream {
    Box::pin(futures_util::stream::iter(events))
}

/// Chat SSE endpoint builder
pub struct ChatSseClient {
    #[cfg(not(target_arch = "wasm32"))]
    client: Client,
    base_url: String,
    auth_token: Option<String>,
}

impl ChatSseClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            client: Client::new(),
            base_url: base_url.into(),
            auth_token: None,
        }
    }

    pub fn with_auth(self, token: String) -> Self {
        Self {
            auth_token: Some(token),
            ..self
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build_stream_request(
        &self,
        request: &ChatRequest,
        request_id: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let url = format!("{}/api/v1/chat", self.base_url);
        let mut req = self
            .client
            .post(&url)
            .header("Accept", "text/event-stream")
            .json(request);
        if let Some(ref token) = self.auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        if let Some(request_id) = request_id {
            req = req.header("x-request-id", request_id);
        }
        req
    }

    pub async fn stream_chat_with_request(
        &self,
        request: ChatRequest,
        request_id: Option<&str>,
    ) -> anyhow::Result<BoxedSseStream> {
        #[cfg(target_arch = "wasm32")]
        {
            use gloo_net::http::Request;

            let url = format!("{}/api/v1/chat", self.base_url);
            let mut http_request = Request::post(&url)
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json");
            if let Some(ref token) = self.auth_token {
                http_request = http_request.header("Authorization", &format!("Bearer {}", token));
            }
            if let Some(request_id) = request_id {
                http_request = http_request.header("x-request-id", request_id);
            }
            let response = http_request
                .body(serde_json::to_string(&request)?)?
                .send()
                .await?;
            if !response.ok() {
                anyhow::bail!("API error: {}", response.status());
            }
            let text = response.text().await?;
            return Ok(sse_stream(parse_sse_events_from_text(&text)));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let resp = self
                .build_stream_request(&request, request_id)
                .send()
                .await?
                .error_for_status()?;
            Ok(sse_stream(resp))
        }
    }

    /// Start a streaming chat session and return a stream of SseEvents.
    pub async fn stream_chat(
        &self,
        notebook_id: &str,
        query: &str,
        session_id: Option<&str>,
        agent_type: &str,
        request_id: Option<&str>,
    ) -> anyhow::Result<BoxedSseStream> {
        self.stream_chat_with_request(
            ChatRequest {
                query: query.to_string(),
                notebook_id: Some(notebook_id.to_string()),
                session_id: session_id.map(String::from),
                agent_type: agent_type.to_string(),
                source_type: None,
                source_token: None,
                doc_scope: vec![],
                messages: vec![],
                stream: true,
            },
            request_id,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatRequest, ChatSseClient, SseEvent, parse_sse_event};
    #[cfg(not(target_arch = "wasm32"))]
    use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderName};

    #[test]
    fn parses_official_start_event() {
        let event = parse_sse_event(
            "start",
            r#"{"event":"start","request_id":"req_123","session_id":"sess_123"}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Start { request_id, session_id })
                if request_id == "req_123" && session_id == "sess_123"
        ));
    }

    #[test]
    fn parses_official_token_event() {
        let event = parse_sse_event(
            "token",
            r#"{"event":"token","request_id":"req_123","message_id":7,"content":"hello"}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Token { request_id, message_id, content })
                if request_id == "req_123" && message_id == 7 && content == "hello"
        ));
    }

    #[test]
    fn parses_official_citations_event() {
        let event = parse_sse_event(
            "citations",
            r#"{"event":"citations","request_id":"req_123","message_id":7,"citations":[{"citation_id":1,"doc_id":"doc-1","doc_name":"Doc"}]}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Citations { request_id, message_id, citations })
                if request_id == "req_123"
                    && message_id == 7
                    && citations.len() == 1
                    && citations[0].get("doc_id").and_then(|value| value.as_str()) == Some("doc-1")
        ));
    }

    #[test]
    fn parses_official_done_event() {
        let event = parse_sse_event(
            "done",
            r#"{"event":"done","request_id":"req_123","session_id":"sess_123","message_id":7,"payload":{"answer":"done text","session_id":"sess_123"}}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Done { request_id, session_id, message_id, payload })
                if request_id == "req_123"
                    && session_id == "sess_123"
                    && message_id == 7
                    && payload.get("answer").and_then(|value| value.as_str()) == Some("done text")
                    && payload.get("session_id").and_then(|value| value.as_str()) == Some("sess_123")
        ));
    }

    #[test]
    fn parses_official_error_event() {
        let event = parse_sse_event(
            "error",
            r#"{"event":"error","request_id":"req_123","code":"bad_request","message":"bad news"}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Error { request_id, code, message })
                if request_id == "req_123" && code == "bad_request" && message == "bad news"
        ));
    }

    #[test]
    fn falls_back_to_event_name_from_message_payload() {
        let event = parse_sse_event(
            "message",
            r#"{"event":"token","request_id":"req_123","message_id":7,"content":"fallback"}"#,
        );
        assert!(matches!(
            event,
            Some(SseEvent::Token { request_id, message_id, content })
                if request_id == "req_123" && message_id == 7 && content == "fallback"
        ));
    }

    #[test]
    fn rejects_legacy_planner_complete_event() {
        let event = parse_sse_event(
            "planner_complete",
            r#"{"event":"planner_complete","payload":{"mode":"rag"}}"#,
        );
        assert!(
            event.is_none(),
            "legacy planner_complete event should no longer be parsed"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_stream_request_builder_sets_transport_request_id_header() {
        let client = ChatSseClient::new("http://example.test").with_auth("token-123".to_string());
        let request = client
            .build_stream_request(
                &ChatRequest {
                    query: "hello".to_string(),
                    notebook_id: Some("nb-1".to_string()),
                    session_id: None,
                    agent_type: "rag".to_string(),
                    source_type: None,
                    source_token: None,
                    doc_scope: Vec::new(),
                    messages: Vec::new(),
                    stream: true,
                },
                Some("req-123"),
            )
            .build()
            .expect("request should build");

        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.url().as_str(), "http://example.test/api/v1/chat");
        assert_eq!(
            request.headers().get(ACCEPT),
            Some(&"text/event-stream".parse().unwrap())
        );
        assert_eq!(
            request
                .headers()
                .get(HeaderName::from_static("x-request-id")),
            Some(&"req-123".parse().unwrap())
        );
        assert_eq!(
            request.headers().get(AUTHORIZATION),
            Some(&"Bearer token-123".parse().unwrap())
        );
    }
}
