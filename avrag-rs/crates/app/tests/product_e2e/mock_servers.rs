//! Mock HTTP servers for Product E2E (LLM, Embedding, Search).

use axum::{
    Json, Router,
    extract::Query,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Names of the canned LLM responses served by [`mock_llm_handler`].
///
/// Tests can pin a call to a specific route by sending the
/// `X-Mock-Route` request header (the production LLM client never
/// sets this header, so production calls always fall through to
/// system-prompt matching).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MockLlmRoute {
    RagPlanner,
    RagCoverageEvaluator,
    RagAnswer,
    SearchPlanner,
    SearchCoverageEvaluator,
    SearchAnswer,
    FormatSkillPpt,
    FormatSkillHtml,
    ChatAnswer,
    Fallback,
}

impl MockLlmRoute {
    /// Return the canned response body for this route.
    fn canned_response(self) -> &'static str {
        match self {
            Self::RagPlanner => {
                r#"{"calls": [{"tool": "dense_retrieval", "version": "1.0", "args": {"queries": ["antifragility Taleb summary"], "modality": "text", "top_k": 10}}], "next_step": "answer"}"#
            }
            Self::RagCoverageEvaluator => {
                r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 3, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#
            }
            Self::RagAnswer => {
                "Based on the document, antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures. The concept was developed by Nassim Nicholas Taleb."
            }
            Self::SearchPlanner => {
                r#"{"sub_queries": ["Tokyo weather today"], "intent_summary": "The user wants to know the current weather in Tokyo.", "needs_clarification": false}"#
            }
            Self::SearchCoverageEvaluator => {
                r#"{"decision": "sufficient", "dimensions": [{"name": "coverage", "attempted": true, "covered": true, "retrieved_count": 1, "query_ids": ["q1"], "status": "covered_strong"}], "next_actions": [], "reasoning": "good"}"#
            }
            Self::SearchAnswer => "The weather in Tokyo today is sunny with a high of 25°C [[1]].",
            Self::FormatSkillPpt => {
                "<html><body><div class=\"slide\"><h1>Slide 1</h1><p>Summary of antifragility</p></div><div class=\"slide\"><h1>Slide 2</h1><p>Key concepts</p></div></body></html>"
            }
            Self::FormatSkillHtml => {
                "<html><body><h1>Antifragility</h1><p>Antifragility is a property of systems that benefit from stress.</p></body></html>"
            }
            Self::ChatAnswer => {
                "Hello! I'm the general chat assistant for Context OS. How can I help you today?"
            }
            Self::Fallback => {
                "This document discusses antifragility, a concept by Nassim Nicholas Taleb describing systems that benefit from shock and disorder."
            }
        }
    }

    /// Resolve a route from the optional `X-Mock-Route` header value.
    /// Returns `None` if the header is missing or has an unknown value.
    pub(crate) fn from_header(value: &str) -> Option<Self> {
        match value.trim() {
            "rag-planner" => Some(Self::RagPlanner),
            "rag-eval" => Some(Self::RagCoverageEvaluator),
            "rag-answer" => Some(Self::RagAnswer),
            "search-planner" => Some(Self::SearchPlanner),
            "search-eval" => Some(Self::SearchCoverageEvaluator),
            "search-answer" => Some(Self::SearchAnswer),
            "format-ppt" => Some(Self::FormatSkillPpt),
            "format-html" => Some(Self::FormatSkillHtml),
            "chat-answer" => Some(Self::ChatAnswer),
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }

    /// Resolve a route by inspecting the system prompt text. This is the
    /// fallback path used by the production LLM client (which does NOT
    /// set `X-Mock-Route`).
    ///
    /// ## Order matters
    ///
    /// The format-skill catalog (`- ppt-generation (v1.0): ...`,
    /// `- html-renderer (v1.0): ...`) is appended to **every** RAG
    /// answer-phase system prompt, so the format-skill checks must
    /// come BEFORE the generic RAG answer check. Same logic for the
    /// search answer: the user prompt template always includes a
    /// `Search results:` line, so the search-answer check must be
    /// early enough to not be masked by later fallbacks.
    pub(crate) fn from_system_prompt(system_prompt: &str, user_prompt: &str) -> Self {
        // 1. RAG planner
        if system_prompt.contains("Context OS RAG retrieval planner") {
            Self::RagPlanner
        // 2. RAG coverage evaluator
        } else if system_prompt.contains("Context OS retrieval coverage evaluator") {
            Self::RagCoverageEvaluator
        // 3. Format skills (must come before RAG answer — the catalog
        //    line is appended to every RAG answer prompt).
        } else if system_prompt.contains("Context OS presentation generation assistant")
            || system_prompt.contains("ppt-generation")
        {
            Self::FormatSkillPpt
        } else if system_prompt.contains("Context OS HTML rendering assistant")
            || system_prompt.contains("html-renderer")
        {
            Self::FormatSkillHtml
        } else if system_prompt.contains("general chat assistant for Context OS") {
            Self::ChatAnswer
        // 4. RAG answer (default for the answer phase)
        } else if system_prompt.contains("Context OS RAG answer agent") {
            Self::RagAnswer
        // 5. Search pipeline
        } else if system_prompt.contains("Context OS Web Search planner") {
            Self::SearchPlanner
        } else if system_prompt.contains("Context OS web-search coverage evaluator") {
            Self::SearchCoverageEvaluator
        } else if system_prompt.contains("Context OS Web Search answer agent")
            || system_prompt.contains("Answer the user's original web-search question")
            || user_prompt.contains("Search results:")
        {
            Self::SearchAnswer
        // 6. Fallback (e.g. summary generation)
        } else {
            Self::Fallback
        }
    }
}
pub(crate) async fn start_mock_llm_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new().route(
        "/chat/completions",
        post(mock_llm_handler).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock llm");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx)
}

/// Start a mock Embedding HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, embedding_should_503_flag, call_count).
pub(crate) async fn start_mock_embedding_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicBool>,
    Arc<AtomicUsize>,
) {
    let embedding_should_503 = Arc::new(AtomicBool::new(false));
    let embedding_call_count = Arc::new(AtomicUsize::new(0));
    let flag = embedding_should_503.clone();
    let call_count = embedding_call_count.clone();

    let app = Router::new().route(
        "/embeddings",
        post(move |req| mock_embedding_handler(req, flag.clone(), call_count.clone())),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock embedding");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, embedding_should_503, embedding_call_count)
}

fn mock_tool_names(req: &serde_json::Value) -> Vec<String> {
    req.get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(|name| name.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mock_native_tool_call(tool_names: &[String], user_prompt: &str) -> Option<serde_json::Value> {
    let query = user_prompt
        .trim()
        .trim_start_matches("[prior_user_query]")
        .trim()
    .to_string();
    let query = if query.is_empty() {
        "context os query".to_string()
    } else {
        query
    };

    if tool_names.iter().any(|name| name == "web_search") {
        return Some(json!({
            "id": "call_web_search_0",
            "type": "function",
            "function": {
                "name": "web_search",
                "arguments": serde_json::to_string(&json!({
                    "query": query,
                    "vertical": "web",
                })).unwrap_or_else(|_| "{}".to_string()),
            }
        }));
    }

    if tool_names.iter().any(|name| name == "dense_retrieval") {
        return Some(json!({
            "id": "call_dense_retrieval_0",
            "type": "function",
            "function": {
                "name": "dense_retrieval",
                "arguments": serde_json::to_string(&json!({
                    "queries": [query],
                    "modality": "text",
                    "top_k": 10,
                    "doc_scope": [],
                })).unwrap_or_else(|_| "{}".to_string()),
            }
        }));
    }

    None
}

fn detect_synthesis_route(system_prompt: &str, messages: &[serde_json::Value]) -> Option<MockLlmRoute> {
    let transcript = messages
        .iter()
        .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    if system_prompt.contains("ppt-generation") {
        return Some(MockLlmRoute::FormatSkillPpt);
    }
    if system_prompt.contains("html-renderer") {
        return Some(MockLlmRoute::FormatSkillHtml);
    }
    if transcript.contains("\"results\"")
        && transcript.contains("\"url\"")
        && (system_prompt.contains("搜索助手") || transcript.contains("web_search"))
    {
        return Some(MockLlmRoute::SearchAnswer);
    }
    if transcript.contains("\"chunk_id\"") || system_prompt.contains("RAG 助手") {
        return Some(MockLlmRoute::RagAnswer);
    }
    None
}

async fn mock_llm_handler(
    headers: axum::http::HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> axum::response::Response {
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let system_prompt = messages
        .iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let user_prompt = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");

    let is_stream = req.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let tool_names = mock_tool_names(&req);
    let has_tool_results = messages
        .iter()
        .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"));

    // ReAct loop: first tools-enabled turn should emit native tool calls.
    if !is_stream && !tool_names.is_empty() && !has_tool_results {
        if let Some(tool_call) = mock_native_tool_call(&tool_names, user_prompt) {
            return axum::Json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [tool_call],
                    }
                }],
                "usage": {"prompt_tokens": 100, "completion_tokens": 1, "total_tokens": 101},
                "model": "mock-llm"
            }))
            .into_response();
        }
    }

    // ReAct loop: after tool results, stop iterating and proceed to synthesis.
    if !is_stream && !tool_names.is_empty() && has_tool_results {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": ""}}],
            "usage": {"prompt_tokens": 50, "completion_tokens": 1, "total_tokens": 51},
            "model": "mock-llm"
        }))
        .into_response();
    }

    // 1. Header-based routing (explicit, takes priority).
    let route = headers
        .get("x-mock-route")
        .and_then(|v| v.to_str().ok())
        .and_then(MockLlmRoute::from_header)
        .or_else(|| detect_synthesis_route(system_prompt, &messages))
        .unwrap_or_else(|| MockLlmRoute::from_system_prompt(system_prompt, user_prompt));

    let content = route.canned_response();

    if is_stream {
        // SSE format expected by ChatCompletionStreamParser.
        // Emit 1-char deltas (token-by-token) so `MessageDelta`
        // events fire frequently and the production `complete_stream`
        // path is exercised end-to-end.
        let mut body = String::new();
        for (i, ch) in content.chars().enumerate() {
            let delta_json = json!({
                "choices": [{
                    "delta": {"content": ch.to_string()},
                    "index": 0
                }],
                "model": "mock-llm"
            });
            body.push_str(&format!("data: {delta_json}\n\n"));
            // Small inter-chunk gap so the client sees multiple chunks
            // (production has variable latency between tokens).
            if i % 8 == 7 {
                body.push_str(": keep-alive\n\n");
            }
        }
        // Final chunk with usage so the parser records it.
        let final_json = json!({
            "choices": [{"delta": {}, "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        });
        body.push_str(&format!("data: {final_json}\n\n"));
        body.push_str("data: [DONE]\n\n");

        axum::response::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .body(axum::body::Body::from(body))
            .unwrap()
    } else {
        axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": content}}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        }))
        .into_response()
    }
}
async fn mock_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
    embedding_call_count: Arc<AtomicUsize>,
) -> axum::response::Response {
    embedding_call_count.fetch_add(1, Ordering::SeqCst);

    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "embedding service unavailable" })),
        )
            .into_response();
    }

    let texts = req["input"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
    // All vectors identical so dense retrieval always returns high similarity.
    let vec: Vec<f32> = (0..dim).map(|j| 0.1_f32 + (j % 10) as f32 * 0.01).collect();
    let data: Vec<serde_json::Value> = texts.iter().map(|_| json!({"embedding": vec})).collect();

    Json(json!({ "data": data, "model": "mock-embedding" })).into_response()
}

/// Start a mock Brave Search HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, search_should_429_flag).
pub(crate) async fn start_mock_search_server() -> (String, tokio::sync::oneshot::Sender<()>, Arc<AtomicBool>) {
    let search_should_429 = Arc::new(AtomicBool::new(false));
    let flag = search_should_429.clone();

    let flag2 = flag.clone();
    let flag3 = flag.clone();
    let app = Router::new()
        .route(
            "/res/v1/llm/context",
            post(move |req| mock_search_handler(req, flag.clone())),
        )
        .route(
            "/res/v1/news/search",
            get(move |Query(params): Query<MockNewsQuery>| async move {
                mock_news_search_handler(params, flag2.clone())
            })
            .post(move |req| mock_search_handler(req, flag3.clone())),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock search");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, search_should_429)
}

#[derive(Debug, serde::Deserialize)]
struct MockNewsQuery {
    q: Option<String>,
}

fn mock_news_search_handler(
    params: MockNewsQuery,
    search_should_429: Arc<AtomicBool>,
) -> axum::response::Response {
    if search_should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let _query = params.q.as_deref().unwrap_or("unknown");
    Json(json!({
        "results": [
            {
                "title": "Tokyo Weather Today",
                "url": "https://example.com/weather-tokyo",
                "description": "Sunny with a high of 25°C in Tokyo today."
            }
        ]
    }))
    .into_response()
}

async fn mock_search_handler(
    Json(req): Json<serde_json::Value>,
    search_should_429: Arc<AtomicBool>,
) -> axum::response::Response {
    if search_should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let _query = req["q"].as_str().unwrap_or("unknown");
    Json(json!({
        "grounding": {
            "generic": [
                {
                    "url": "https://example.com/weather-tokyo",
                    "title": "Tokyo Weather Today",
                    "snippets": ["Sunny with a high of 25°C in Tokyo today."]
                }
            ],
            "map": []
        },
        "sources": {
            "https://example.com/weather-tokyo": {
                "title": "Tokyo Weather Today",
                "hostname": "example.com"
            }
        }
    }))
    .into_response()
}
