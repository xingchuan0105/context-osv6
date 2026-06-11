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

pub use common::{ChatResponse, Citation, DegradeReason, DegradeTraceItem, DocumentStatus};

/// A trace event record that carries reasoning text (plan_decision, evaluation, etc.).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceReasoningRecord {
    pub stage: String,
    pub reasoning: serde_json::Value,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub detail: serde_json::Value,
}

/// Observability capture from an SSE chat stream (reasoning deltas + trace events).
#[derive(Debug, Clone)]
pub struct StreamReasoningCapture {
    pub summary: String,
    pub delta_count: usize,
    pub trace_reasoning: Vec<TraceReasoningRecord>,
    pub prompt_snapshots: Vec<serde_json::Value>,
}

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
    ChatStreamParams, DEFAULT_TEST_ORG_ID, DEFAULT_TEST_USER_ID, TestContext, unique_test_identity,
};

#[cfg(test)]
mod mock_routing_tests {
    use super::mock_servers::MockLlmRoute;

    #[test]
    fn header_routing_recognizes_all_known_routes() {
        for value in [
            "rag-answer",
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
    fn synthesis_contract_routes_before_format_skill_catalog() {
        // Synthesis system prompts may include both internal_answer_v1 and
        // format-skill catalog lines; contract detection must win so mock
        // E2E returns JSON citations instead of HTML slides.
        let prompt = "\
You are the Context OS RAG answer agent.
Respond with ONLY a JSON object (no markdown fences):
{\"schema_version\":\"internal_answer_v1\",\"answer_text\":\"...\",\"citations\":[]}

## Available Output Formats

- ppt-generation (v1.0): Load when the user requests a slide deck
";
        let route = MockLlmRoute::from_system_prompt(prompt, "");
        assert_eq!(
            route,
            MockLlmRoute::RagAnswer,
            "internal_answer_v1 contract must route to RagAnswer, not format skills"
        );
    }

    #[test]
    fn rag_retrieve_round_codegen_before_synthesis() {
        let codegen = super::mock_servers::format_mock_rag_codegen_response(
            "00000000-0000-4000-8000-000000000001",
        );
        assert!(codegen.contains("<code language=\"python\">"));
        assert!(codegen.contains("client.dense_search"));
        assert!(codegen.contains("json.dumps(chunks)"));
    }

    #[test]
    fn multiround_codegen_scripts_doc_profile_then_chunk_fetch() {
        let doc_id = "2724017d-862d-448a-837e-406cd2f438b4";
        let chunk_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let profile = super::mock_servers::format_mock_rag_doc_profile_codegen(doc_id);
        let fetch = super::mock_servers::format_mock_rag_chunk_fetch_codegen(chunk_id);
        assert!(profile.contains("client.doc_profile"));
        assert!(profile.contains(doc_id));
        assert!(fetch.contains("client.chunk_fetch"));
        assert!(fetch.contains(chunk_id));
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
