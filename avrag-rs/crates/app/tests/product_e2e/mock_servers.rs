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
        } else if system_prompt.contains("general chat assistant for Context OS") {
            Self::ChatAnswer
        // 3. Synthesis answer contracts (before format-skill catalog lines).
        } else if system_prompt.contains("Context OS RAG answer agent")
            || system_prompt.contains("internal_answer_v1")
        {
            Self::RagAnswer
        } else if system_prompt.contains("Context OS Web Search answer agent")
            || system_prompt.contains("internal_search_answer_v1")
            || user_prompt.contains("Search results:")
        {
            Self::SearchAnswer
        // 4. Format skills (catalog appears in synthesis system prompts).
        } else if system_prompt.contains("Context OS presentation generation assistant")
            || system_prompt.contains("ppt-generation")
        {
            Self::FormatSkillPpt
        } else if system_prompt.contains("Context OS HTML rendering assistant")
            || system_prompt.contains("html-renderer")
        {
            Self::FormatSkillHtml
        // 5. Search planner / evaluator
        } else if system_prompt.contains("Context OS Web Search planner") {
            Self::SearchPlanner
        } else if system_prompt.contains("Context OS web-search coverage evaluator") {
            Self::SearchCoverageEvaluator
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

    (
        base_url,
        abort_tx,
        embedding_should_503,
        embedding_call_count,
    )
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

fn is_placeholder_chunk_id(id: &str) -> bool {
    let trimmed = id.trim().trim_matches('"');
    if trimmed.is_empty() || trimmed == "null" {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "..." | "chunk_id" | "uuid" | "mock-chunk-1" | "ch" | "chunk-uuid"
    ) || lower.starts_with('<')
        || trimmed.contains("...")
        || trimmed.contains("CHUNK_ID")
}

fn looks_like_chunk_uuid(id: &str) -> bool {
    let id = id.trim().trim_matches('"');
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn collect_chunk_ids_from_text(text: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("chunk_id") {
        let tail = &rest[start..];
        let after_key = tail.strip_prefix("chunk_id").unwrap_or(tail);
        let after_colon = after_key
            .split_once(':')
            .map(|(_, v)| v)
            .unwrap_or(after_key);
        let trimmed = after_colon.trim().trim_matches('"');
        if !trimmed.is_empty() && trimmed.len() <= 128 {
            let id = trimmed
                .split(|c: char| c == '"' || c == ',' || c == '}' || c.is_whitespace())
                .next()
                .unwrap_or(trimmed);
            if !id.is_empty() && !is_placeholder_chunk_id(id) {
                out.push(id.to_string());
            }
        }
        rest = &rest[start + 8..];
    }
}

fn first_chunk_id_in_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(id) = item
                    .get("chunk_id")
                    .and_then(|v| v.as_str())
                    .filter(|id| !is_placeholder_chunk_id(id))
                {
                    return Some(id.to_string());
                }
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(chunks) = map.get("chunks") {
                if let Some(id) = first_chunk_id_in_value(chunks) {
                    return Some(id);
                }
            }
            for v in map.values() {
                if let Some(id) = first_chunk_id_in_value(v) {
                    return Some(id);
                }
            }
        }
        _ => {}
    }
    None
}

fn extract_chunk_id_from_tool_results_block(transcript: &str) -> Option<String> {
    let start = transcript.find("<tool_results>")?;
    let after = &transcript[start + "<tool_results>".len()..];
    let end = after.find("</tool_results>")?;
    let inner = after[..end].trim();
    let parsed = serde_json::from_str::<serde_json::Value>(inner).ok()?;
    if let serde_json::Value::Array(tool_results) = parsed {
        for result in tool_results {
            if let Some(data) = result.get("data") {
                if let Some(id) = first_chunk_id_in_value(data) {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn extract_retrieval_chunk_id(transcript: &str) -> Option<String> {
    if let Some(id) = extract_chunk_id_from_tool_results_block(transcript) {
        return Some(id);
    }

    let preferred_sections = [
        "自动兜底检索结果:",
        "自动兜底检索结果",
        "<code_execution_result>",
    ];
    for marker in preferred_sections {
        let Some(idx) = transcript.find(marker) else {
            continue;
        };
        let mut ids = Vec::new();
        collect_chunk_ids_from_text(&transcript[idx..], &mut ids);
        if let Some(id) = ids.iter().find(|id| looks_like_chunk_uuid(id)) {
            return Some(id.clone());
        }
        if let Some(id) = ids.into_iter().find(|id| !is_placeholder_chunk_id(id)) {
            return Some(id);
        }
    }
    None
}

fn mock_synthesis_json_rag(transcript: &str) -> String {
    let chunk_id = extract_retrieval_chunk_id(transcript).unwrap_or_else(|| {
        let preview: String = transcript.chars().take(500).collect();
        panic!("mock RAG synthesis could not resolve chunk_id; transcript preview: {preview}")
    });
    let answer_text = format!(
        "Based on the document, antifragility is a property of systems that benefit from stress and disorder [[cite:{chunk_id}]]. The concept was developed by Nassim Nicholas Taleb."
    );
    serde_json::json!({
        "schema_version": "internal_answer_v1",
        "answer_text": answer_text,
        "citations": [{"chunk_id": chunk_id}],
        "coverage": "full",
        "refusal_reason": null
    })
    .to_string()
}

fn mock_synthesis_json_search() -> String {
    serde_json::json!({
        "schema_version": "internal_search_answer_v1",
        "answer_text": "The weather in Tokyo today is sunny with a high of 25°C [[1]].",
        "citations": [{"index": 1}],
        "coverage": "full",
        "refusal_reason": null
    })
    .to_string()
}

fn resolve_mock_content(route: MockLlmRoute, system_prompt: &str, transcript: &str) -> String {
    match route {
        MockLlmRoute::RagAnswer if system_prompt.contains("internal_answer_v1") => {
            mock_synthesis_json_rag(transcript)
        }
        MockLlmRoute::SearchAnswer if system_prompt.contains("internal_search_answer_v1") => {
            mock_synthesis_json_search()
        }
        _ => route.canned_response().to_string(),
    }
}

fn detect_synthesis_route(
    system_prompt: &str,
    _messages: &[serde_json::Value],
) -> Option<MockLlmRoute> {
    // Synthesis contract must win over format-skill catalog lines in the same system prompt.
    if system_prompt.contains("internal_search_answer_v1")
        || system_prompt.contains("Context OS Web Search answer agent")
    {
        return Some(MockLlmRoute::SearchAnswer);
    }
    if system_prompt.contains("internal_answer_v1")
        || system_prompt.contains("Context OS RAG answer agent")
    {
        return Some(MockLlmRoute::RagAnswer);
    }
    if system_prompt.contains("ppt-generation") {
        return Some(MockLlmRoute::FormatSkillPpt);
    }
    if system_prompt.contains("html-renderer") {
        return Some(MockLlmRoute::FormatSkillHtml);
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

    let is_synthesis_contract = system_prompt.contains("internal_answer_v1")
        || system_prompt.contains("internal_search_answer_v1");
    // ReAct retrieve without native tools (RAG codegen / Search post-tool): end loop quickly.
    if !is_stream
        && tool_names.is_empty()
        && !is_synthesis_contract
        && (system_prompt.contains("检索 → 评估 → 合成")
            || system_prompt.contains("搜索 → 验证 → 合成"))
    {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": ""}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": 1, "total_tokens": 41},
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

    let transcript = messages
        .iter()
        .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = resolve_mock_content(route, system_prompt, &transcript);
    let emit_mock_reasoning = matches!(
        route,
        MockLlmRoute::RagAnswer
            | MockLlmRoute::SearchAnswer
            | MockLlmRoute::ChatAnswer
            | MockLlmRoute::FormatSkillHtml
            | MockLlmRoute::FormatSkillPpt
    );

    if is_stream {
        // SSE format expected by ChatCompletionStreamParser.
        // Emit 1-char deltas (token-by-token) so `MessageDelta`
        // events fire frequently and the production `complete_stream`
        // path is exercised end-to-end.
        let mut body = String::new();
        if emit_mock_reasoning {
            for ch in "Mock stream reasoning. ".chars() {
                let delta_json = json!({
                    "choices": [{
                        "delta": {"reasoning_content": ch.to_string()},
                        "index": 0
                    }],
                    "model": "mock-llm"
                });
                body.push_str(&format!("data: {delta_json}\n\n"));
            }
        }
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
        let mut message = json!({"role": "assistant", "content": content});
        if emit_mock_reasoning && !content.is_empty() {
            message["reasoning_content"] = json!("Mock model reasoning for offline E2E.");
        }
        axum::Json(json!({
            "choices": [{"message": message}],
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
pub(crate) async fn start_mock_search_server()
-> (String, tokio::sync::oneshot::Sender<()>, Arc<AtomicBool>) {
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
