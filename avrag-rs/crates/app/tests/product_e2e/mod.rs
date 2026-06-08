//! Product E2E shared infrastructure.
//!
//! Design principles:
//! - HTTP black-box entry only — no direct Strategy/Runtime calls.
//! - Smoke uses real PG + local Object Store, mocks LLM/Search/Embedding via HTTP-level stubs.
//! - Protocol assertions first, then deserialize to business types.

pub mod assertions;
pub mod setup;

pub mod failure;
pub mod integration;
pub mod llm_real;
pub mod smoke;
pub mod tenants;

mod mock_servers;
mod test_context;

/// Raw HTTP response from the test client.
///
/// All `ctx.chat()` / `ctx.upload_document()` helpers return this first.
/// Protocol-layer assertions operate on this type.
/// Product-layer assertions require deserializing `body_json` first.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body_json: serde_json::Value,
}

impl HttpResponse {
    /// Deserialize the JSON body into a typed business response.
    pub fn into_business<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.body_json)
    }
}

// ---------------------------------------------------------------------------
// Server-Sent Events (SSE) parsing
// ---------------------------------------------------------------------------

/// A single SSE event parsed from a streaming chat response.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event name from the `event: <name>` line.
    pub event: String,
    /// JSON body from the `data: <json>` line.
    pub data: serde_json::Value,
}

/// Minimal SSE parser. Feed it raw response bytes via [`SseParser::feed`];
/// it returns any complete events it finds. Handles `event:` / `data:`
/// lines, blank-line event terminators, and `:` comment lines.
pub struct SseParser {
    buf: String,
    current_event: Option<String>,
    current_data: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            current_event: None,
            current_data: None,
        }
    }

    /// Feed a chunk of bytes; return any complete events parsed from it.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        use std::str::from_utf8;
        let s = match from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.buf.push_str(s);
        let mut out = Vec::new();
        while let Some(idx) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=idx).collect();
            let line = line.trim_end_matches(&['\r', '\n'][..]).to_string();
            if line.is_empty() {
                // Event terminator: emit if we have data
                if let (Some(event), Some(data)) =
                    (self.current_event.take(), self.current_data.take())
                {
                    let parsed =
                        serde_json::from_str(&data).unwrap_or(serde_json::Value::String(data));
                    out.push(SseEvent {
                        event,
                        data: parsed,
                    });
                }
            } else if let Some(rest) = line.strip_prefix("event:") {
                self.current_event = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                // Spec: concatenate multiple data: lines with \n
                let piece = rest.trim_start();
                match &mut self.current_data {
                    Some(d) => {
                        d.push('\n');
                        d.push_str(piece);
                    }
                    None => {
                        self.current_data = Some(piece.to_string());
                    }
                }
            } else if line.starts_with(':') {
                // Comment / keep-alive; ignore
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Business response types (re-exported from production code)
// ---------------------------------------------------------------------------

pub use common::{ChatResponse, Citation, DegradeTraceItem, DocumentStatus};

// ---------------------------------------------------------------------------
// Upload response (document upload)
// ---------------------------------------------------------------------------

/// Response from `POST /api/v1/notebooks/{id}/documents`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadResponse {
    pub document_id: String,
    pub notebook_id: String,
    pub upload_url: String,
    #[serde(default)]
    pub status: u16,
}

/// Notebook creation response wrapper.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotebookResponse {
    pub notebook: NotebookInner,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotebookInner {
    pub id: String,
    pub title: String,
}


pub use test_context::{
    TestContext, DEFAULT_TEST_ORG_ID, DEFAULT_TEST_USER_ID, unique_test_identity,
};

#[cfg(test)]
mod mock_routing_tests {
    use super::mock_servers::MockLlmRoute;

    #[test]
    fn header_routing_recognizes_all_known_routes() {
        for value in [
            "rag-planner",
            "rag-eval",
            "rag-answer",
            "search-planner",
            "search-eval",
            "search-answer",
            "format-ppt",
            "format-html",
            "chat-answer",
            "fallback",
        ] {
            assert!(
                MockLlmRoute::from_header(value).is_some(),
                "header value '{value}' should map to a route"
            );
        }
    }

    #[test]
    fn header_routing_returns_none_for_unknown_value() {
        assert_eq!(MockLlmRoute::from_header(""), None);
        assert_eq!(MockLlmRoute::from_header("garbage"), None);
        assert_eq!(MockLlmRoute::from_header("RAG-PLANNER"), None); // case-sensitive
    }

    #[test]
    fn system_prompt_routing_orders_format_skills_before_rag_answer() {
        // The RAG answer phase appends the format-skill catalog to the
        // system prompt, so the system prompt contains BOTH the RAG
        // answer marker AND the format-skill IDs. The format skill
        // must win; if we ever re-order and put RAG answer first, the
        // format_output integration tests will start failing with
        // 'expected slide in formatted answer'.
        let prompt = "\
You are the Context OS RAG answer agent.

## Available Output Formats

- ppt-generation (v1.0): Load when the user requests a slide deck
- html-renderer (v1.0): Load when the user asks for HTML

## Selected Format Skills

You are the Context OS presentation generation assistant.
When the user asks for a presentation, output structured JSON.
";
        let route = MockLlmRoute::from_system_prompt(prompt, "");
        assert_eq!(
            route,
            MockLlmRoute::FormatSkillPpt,
            "format-skill catalog in RAG answer prompt must route to PPT, not generic RAG answer"
        );
    }

    #[test]
    fn system_prompt_routing_picks_rag_planner_for_planner_marker() {
        let prompt = "You are the Context OS RAG retrieval planner. Given a query, decompose it into tool calls.";
        assert_eq!(
            MockLlmRoute::from_system_prompt(prompt, ""),
            MockLlmRoute::RagPlanner
        );
    }

    #[test]
    fn system_prompt_routing_falls_back_when_no_marker_matches() {
        let prompt = "You are a generic helpful assistant.";
        let user = "Hello";
        assert_eq!(
            MockLlmRoute::from_system_prompt(prompt, user),
            MockLlmRoute::Fallback
        );
    }
}
