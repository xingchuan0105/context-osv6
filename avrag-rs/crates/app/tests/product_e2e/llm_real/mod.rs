//! Real-LLM E2E regression suite.
//!
//! These tests use production LLM providers instead of the mock LLM server.
//! They are marked `#[ignore]` by default because they:
//! - incur API cost (DeepSeek + DashScope),
//! - are non-deterministic,
//! - are slower than mock tests,
//! - may hit provider rate limits under parallel execution.
//!
//! Run serially with:
//!   E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1 --nocapture
//!
//! PDF ingest RAG (`pdf_corpus`, `pdf_rag_e2e`) uses P4 LiteParse hybrid routing on bundled
//! `phase0-mini.pdf` by default; txt RAG (`rag_real`, etc.) uses `antifragile.txt` local parse.
//!
//! Artifacts: `crates/app/tests/e2e_output/llm_real/{run_id}/{test_name}/`
//!   - `response.json`, `reasoning_summary.txt`, `trace_reasoning.jsonl`, `prompt_snapshots.json`, `metadata.json`
//! Streaming requests use `debug: true` so prompt_snapshot trace events are emitted.
//! `trace_reasoning.jsonl` comes from loop telemetry (PlanDecision / Evaluation), not LLM eval.
//! `metadata.reasoning_empty_warning` is true only when both summary and trace_reasoning are empty
//! (usually indicates dropped SSE traces, not a non-thinking model).
//! `metadata.stream_error_with_done` flags final-attempt error+done coexistence.
//!
//! Gated by `E2E_MODE=nightly` (or `llm_real`) via [`require_nightly_suite`].
//!
//! Required environment (loaded from the repository `.env` if not already set):
//!   AGENT_LLM_BASE_URL, AGENT_LLM_API_KEY, AGENT_LLM_MODEL
//!   MEMORY_LLM_BASE_URL, MEMORY_LLM_API_KEY, MEMORY_LLM_MODEL
//!   INGESTION_LLM_BASE_URL, INGESTION_LLM_API_KEY, INGESTION_LLM_MODEL
//!   EMBEDDING_BASE_URL, EMBEDDING_API_KEY, EMBEDDING_MODEL
//!   SEARCH_PROVIDER, SEARCH_BASE_URL, SEARCH_API_KEY (search tests only)
//!   SEARCH_REQUIRE_REAL=1 — fail instead of mock fallback when Brave unreachable (search_real sets this)

use crate::product_e2e::{
    ChatResponse, ChatStreamParams, SseEvent, StreamReasoningCapture, TestContext,
    TraceReasoningRecord,
};

pub(crate) use crate::product_e2e::e2e_gate::require_nightly_suite;

/// Load key/value pairs from the repository `.env` file into the process
/// environment.  This lets real-LLM tests discover credentials without
/// requiring the caller to `source .env` first.
///
/// Only sets variables that are **not** already present in the environment,
/// so explicit exports take priority.
pub(crate) fn load_env_from_repo_dotenv() {
    // The worktree usually does not have its own `.env`.
    // Try the worktree location first, then fall back to the main repo copy.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/app -> crates -> avrag-rs -> e2e-analyzer -> worktrees -> .claude -> context-osv6 -> avrag-rs/.env
    let main_repo_dotenv = manifest
        .join("../../../../../../avrag-rs/.env")
        .canonicalize()
        .ok();
    let worktree_dotenv = manifest.join("../../.env").canonicalize().ok();
    let path = worktree_dotenv
        .or(main_repo_dotenv)
        .expect("repository .env file must exist for real-LLM tests");

    let content = std::fs::read_to_string(path).expect("read .env");
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let raw_value = raw_value.trim();
        // Strip surrounding quotes if present.
        let value = if raw_value.len() >= 2 {
            let first = raw_value.chars().next().unwrap();
            let last = raw_value.chars().last().unwrap();
            if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
                &raw_value[1..raw_value.len() - 1]
            } else {
                raw_value
            }
        } else {
            raw_value
        };
        let should_set = match std::env::var(key) {
            Err(_) => true,
            // Allow .env to replace placeholder search credentials left in the shell.
            Ok(existing)
                if (key == "SEARCH_API_KEY" && (existing.is_empty() || existing == "mock"))
                    || (key == "SEARCH_BASE_URL" && existing.starts_with("http://127.0.0.1")) =>
            {
                true
            }
            Ok(_) => false,
        };
        if should_set {
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// True when SEARCH_API_KEY (or E2E_BRAVE_API_KEY) is present and not the mock placeholder.
pub fn has_real_search_credentials() -> bool {
    fn valid_key(key: Result<String, std::env::VarError>) -> bool {
        key.map(|k| !k.is_empty() && k != "mock").unwrap_or(false)
    }
    valid_key(std::env::var("SEARCH_API_KEY")) || valid_key(std::env::var("E2E_BRAVE_API_KEY"))
}

/// Ensure SEARCH_PROVIDER / SEARCH_BASE_URL defaults when using real Brave.
pub fn ensure_search_defaults() {
    if !std::env::var("SEARCH_API_KEY")
        .map(|k| !k.is_empty() && k != "mock")
        .unwrap_or(false)
    {
        if let Ok(key) = std::env::var("E2E_BRAVE_API_KEY") {
            if !key.is_empty() {
                unsafe { std::env::set_var("SEARCH_API_KEY", key) };
            }
        }
    }
    if !std::env::var("SEARCH_PROVIDER")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        unsafe { std::env::set_var("SEARCH_PROVIDER", "brave_llm_context") };
    }
    if !std::env::var("SEARCH_BASE_URL")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        unsafe { std::env::set_var("SEARCH_BASE_URL", "https://api.search.brave.com") };
    }
}

/// Max attempts for non-deterministic real-LLM chat/search calls.
pub const REAL_LLM_MAX_ATTEMPTS: usize = 2;

/// Extra attempts for multi-tool RAG probes where the model may answer in one retrieval pass.
pub const REAL_LLM_MULTITOOL_MAX_ATTEMPTS: usize = 3;

/// Stream deadline for real-LLM chat/rag/search (thinking models can be slow).
pub const REAL_LLM_STREAM_DEADLINE: std::time::Duration = std::time::Duration::from_secs(180);

/// Write multi-phase (research + skeleton + draft + refine) + real Search — align with UI journey.
pub const WRITE_REAL_STREAM_DEADLINE: std::time::Duration = std::time::Duration::from_secs(600);

/// Safety cap on SSE events (thinking models emit many small deltas).
pub const REAL_LLM_STREAM_MAX_EVENTS: usize = 4096;

const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// Real-LLM chat result with streamed reasoning capture for offline analysis.
#[derive(Debug, Clone)]
pub struct LlmRealChatResult {
    pub resp: ChatResponse,
    pub reasoning: StreamReasoningCapture,
    /// True when the final attempt had both an SSE `error` event and a `done` payload.
    pub stream_error_with_done: bool,
}

/// Collect reasoning deltas, trace reasoning, and prompt snapshots from SSE events.
pub fn collect_observability_from_events(events: &[SseEvent]) -> StreamReasoningCapture {
    let mut summary = String::new();
    let mut delta_count = 0usize;
    let mut trace_reasoning = Vec::new();
    let mut prompt_snapshots = Vec::new();

    for event in events {
        match event.event.as_str() {
            "reasoning_summary_delta" => {
                if let Some(chunk) = event.data.get("content").and_then(|v| v.as_str()) {
                    summary.push_str(chunk);
                    delta_count += 1;
                }
            }
            "trace" => {
                let stage = event
                    .data
                    .get("stage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let detail = event
                    .data
                    .get("detail")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                if stage == "prompt_snapshot" {
                    prompt_snapshots.push(detail.clone());
                }

                if let Some(reasoning) = detail.get("reasoning").and_then(|v| v.as_str()) {
                    if !reasoning.is_empty() {
                        trace_reasoning.push(TraceReasoningRecord {
                            stage: stage.clone(),
                            reasoning: serde_json::Value::String(reasoning.to_string()),
                            detail: detail.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    StreamReasoningCapture {
        summary,
        delta_count,
        trace_reasoning,
        prompt_snapshots,
    }
}

/// Concatenate all `reasoning_summary_delta` chunks from an SSE event stream.
pub fn collect_reasoning_summary_from_events(events: &[SseEvent]) -> StreamReasoningCapture {
    collect_observability_from_events(events)
}

/// Merge per-test `extra` fields with stream observability warnings from the result.
pub fn merge_llm_real_extra(
    result: &LlmRealChatResult,
    extra: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut obj = extra
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if result.stream_error_with_done {
        obj.insert("stream_error_with_done".into(), serde_json::json!(true));
    }
    if obj.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(obj))
    }
}

/// Parse the terminal `done` payload into a typed [`ChatResponse`].
pub fn parse_chat_response_from_stream_events(events: &[SseEvent]) -> Option<ChatResponse> {
    for event in events.iter().rev() {
        if event.event != "done" {
            continue;
        }
        let payload = event
            .data
            .get("payload")
            .cloned()
            .unwrap_or_else(|| event.data.clone());
        if let Ok(resp) = serde_json::from_value::<ChatResponse>(payload) {
            return Some(resp);
        }
    }
    None
}

fn stream_had_error(events: &[SseEvent]) -> bool {
    events.iter().any(|e| e.event == "error")
}

/// How observability was captured for a RAG quality probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservabilityMode {
    /// Streaming SSE with `debug: true` — full trace + prompt snapshots.
    FullStream,
    /// Non-streaming fallback when the stream lacked a parseable `done` payload.
    FallbackNonStream,
}

/// RAG probe result with full observability capture for smoke / quality eval.
#[derive(Debug, Clone)]
pub struct RagObservableProbeResult {
    pub resp: ChatResponse,
    pub capture: StreamReasoningCapture,
    pub sse_events: Vec<SseEvent>,
    pub observability_mode: ObservabilityMode,
    pub stream_error_with_done: bool,
}

/// Best-effort snapshot of the local API server's liveness, taken at failure time.
///
/// `GET /health` is an unauthenticated route on the test router (it delegates to the
/// production `transport_http::build_router`). A failed probe leaves this snapshot so an
/// offline reader can tell whether the API process was still accepting connections when
/// reqwest reported `error sending request for url`.
#[derive(Debug, Clone)]
pub struct LivenessSnapshot {
    /// RFC3339 timestamp of the probe.
    pub checked_at: String,
    /// HTTP status code, or `None` if the request itself errored.
    pub status_code: Option<u16>,
    /// First 200 chars of the response body (or the request error text).
    pub body_prefix: String,
    /// Elapsed wall time in milliseconds.
    pub elapsed_ms: u64,
}

/// Probe failure carrying full diagnostics so the root cause can be read offline.
///
/// Replaces the previous `format!("chat: {e}")` that flattened the reqwest error chain and
/// discarded the SSE events captured before the stream died. See
/// `prompts/_backups/doc_chunks_e2e_handoff.md` §3.5 for the failure that motivated this.
#[derive(Debug, Clone)]
pub struct RagObservableProbeFailure {
    /// Full anyhow error chain (`{e:#}`), not the flattened one-liner.
    pub error_chain: String,
    /// Coarse classification: "connect" | "timeout" | "reset" | "other".
    pub error_category: String,
    /// SSE events collected before the stream failed (empty if attempt 1 itself errored).
    pub sse_events: Vec<SseEvent>,
    /// Reasoning capture from the (failed) stream.
    pub capture: StreamReasoningCapture,
    /// Which step failed last: "fallback_non_stream" (the only return-Err path).
    pub failing_stage: String,
}

/// Classify a reqwest error message into a coarse category for triage. Keyword-based;
/// order matters (most specific first).
fn classify_reqwest_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("deadline") {
        "timeout"
    } else if lower.contains("connection reset")
        || lower.contains("reset by peer")
        || lower.contains("broken pipe")
        || lower.contains("connection aborted")
    {
        "reset"
    } else if lower.contains("connect") || lower.contains("connection refused") {
        "connect"
    } else {
        "other"
    }
}

fn empty_reasoning_capture() -> StreamReasoningCapture {
    StreamReasoningCapture {
        summary: String::new(),
        delta_count: 0,
        trace_reasoning: Vec::new(),
        prompt_snapshots: Vec::new(),
    }
}

/// Collect distinct tool names from SSE `tool_result.*` traces and final `tool_results`.
pub fn summarize_tool_activity(events: &[SseEvent], resp: &ChatResponse) -> Vec<String> {
    let mut tools = std::collections::BTreeSet::new();
    for event in events {
        if event.event != "trace" {
            continue;
        }
        let Some(stage) = event.data.get("stage").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(tool) = stage.strip_prefix("tool_result.") {
            tools.insert(tool.to_string());
        }
    }
    for result in &resp.tool_results {
        tools.insert(result.tool.clone());
    }
    tools.into_iter().collect()
}

/// Count SSE trace events whose `stage` equals or starts with `stage`.
pub fn count_sse_trace_stage(events: &[SseEvent], stage: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            event.event == "trace"
                && event
                    .data
                    .get("stage")
                    .and_then(|v| v.as_str())
                    .map(|s| s == stage || s.starts_with(&format!("{stage}.")))
                    .unwrap_or(false)
        })
        .count()
}

/// Streaming RAG chat with full observability for quality probes.
///
/// Uses `debug: true`, `pin_mock_chunk_ids: false` (real retrieval). Retries the
/// stream up to [`REAL_LLM_MAX_ATTEMPTS`] when the terminal `done` payload is missing;
/// falls back to non-streaming chat if streaming still fails. When the stream carries
/// an SSE `error` event (e.g. the LLM provider aborted a Chinese query mid-stream),
/// the product side has already signalled failure, so we skip the retry and fall back
/// immediately instead of burning another attempt.
pub async fn chat_rag_observable_probe(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
) -> Result<RagObservableProbeResult, RagObservableProbeFailure> {
    let params = ChatStreamParams {
        query,
        agent_type: "rag",
        workspace_id,
        doc_scope,
        session_id: None,
        format_hint: None,
        debug: true,
        pin_mock_chunk_ids: false,
    };

    let mut last_events = Vec::new();
    let mut last_capture = empty_reasoning_capture();
    let mut stream_error_with_done = false;

    for attempt in 1..=REAL_LLM_MAX_ATTEMPTS {
        match chat_stream_once(ctx, &params).await {
            Ok((events, Some(resp), capture)) => {
                if stream_had_error(&events) && attempt == REAL_LLM_MAX_ATTEMPTS {
                    stream_error_with_done = true;
                }
                return Ok(RagObservableProbeResult {
                    resp,
                    capture,
                    sse_events: events,
                    observability_mode: ObservabilityMode::FullStream,
                    stream_error_with_done,
                });
            }
            Ok((events, None, capture)) => {
                if stream_had_error(&events) {
                    eprintln!(
                        "[rag_observable] attempt {attempt}/{} stream error event without done; skipping retry, falling back; events={}",
                        REAL_LLM_MAX_ATTEMPTS,
                        events.len()
                    );
                    last_events = events;
                    last_capture = capture;
                    break;
                }
                eprintln!(
                    "[rag_observable] attempt {attempt}/{} missing done payload; events={}",
                    REAL_LLM_MAX_ATTEMPTS,
                    events.len()
                );
                last_events = events;
                last_capture = capture;
            }
            Err(err) => {
                eprintln!(
                    "[rag_observable] attempt {attempt}/{} stream error: {err}",
                    REAL_LLM_MAX_ATTEMPTS
                );
            }
        }
        if attempt < REAL_LLM_MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    eprintln!("[rag_observable] falling back to non-streaming chat");
    let http_resp = match ctx
        .chat_without_mock_chunk_pin(query, workspace_id, doc_scope)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let chain = format!("{e:#}");
            let category = classify_reqwest_error(&chain);
            eprintln!("[rag_observable] fallback non-stream failed ({category}): {chain}");
            return Err(RagObservableProbeFailure {
                error_chain: chain,
                error_category: category.to_string(),
                sse_events: last_events,
                capture: last_capture,
                failing_stage: "fallback_non_stream".to_string(),
            });
        }
    };
    let chat = match http_resp.into_business::<ChatResponse>() {
        Ok(c) => c,
        Err(e) => {
            let chain = format!("response parse failed: {e}");
            let category = classify_reqwest_error(&chain);
            eprintln!("[rag_observable] fallback parse failed ({category}): {chain}");
            return Err(RagObservableProbeFailure {
                error_chain: chain,
                error_category: category.to_string(),
                sse_events: last_events,
                capture: last_capture,
                failing_stage: "fallback_non_stream_parse".to_string(),
            });
        }
    };
    Ok(RagObservableProbeResult {
        resp: chat,
        capture: last_capture,
        sse_events: last_events,
        observability_mode: ObservabilityMode::FallbackNonStream,
        stream_error_with_done: false,
    })
}

/// Best-effort liveness probe of the local API server at failure time.
///
/// Hits the unauthenticated `GET /health` route with a short 5s timeout and records the
/// status code, a body prefix, and elapsed time. Never returns `Err`: any request failure
/// is captured into the snapshot so an offline reader can distinguish "API down" from
/// "API slow / returning errors".
pub async fn probe_api_liveness(ctx: &TestContext) -> LivenessSnapshot {
    let url = format!("{}/health", ctx.base_url);
    let started = std::time::Instant::now();
    let checked_at = chrono::Utc::now().to_rfc3339();

    let req = ctx
        .http_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5));
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            LivenessSnapshot {
                checked_at,
                status_code: Some(status),
                body_prefix: body.chars().take(200).collect(),
                elapsed_ms: started.elapsed().as_millis() as u64,
            }
        }
        Err(e) => {
            let msg = format!("{e}");
            LivenessSnapshot {
                checked_at,
                status_code: None,
                body_prefix: msg.chars().take(200).collect(),
                elapsed_ms: started.elapsed().as_millis() as u64,
            }
        }
    }
}

async fn chat_stream_once(
    ctx: &TestContext,
    params: &ChatStreamParams<'_>,
) -> anyhow::Result<(Vec<SseEvent>, Option<ChatResponse>, StreamReasoningCapture)> {
    let events = ctx
        .chat_stream_with_params(
            ChatStreamParams {
                query: params.query,
                agent_type: params.agent_type,
                workspace_id: params.workspace_id,
                doc_scope: params.doc_scope,
                session_id: params.session_id,
                format_hint: params.format_hint,
                debug: params.debug,
                pin_mock_chunk_ids: params.pin_mock_chunk_ids,
            },
            REAL_LLM_STREAM_MAX_EVENTS,
            REAL_LLM_STREAM_DEADLINE,
        )
        .await?;
    let capture = collect_observability_from_events(&events);
    let resp = parse_chat_response_from_stream_events(&events);
    Ok((events, resp, capture))
}

async fn chat_stream_with_retry_inner(
    ctx: &TestContext,
    params: ChatStreamParams<'_>,
    ready: impl Fn(&ChatResponse) -> bool,
    label: &str,
    max_attempts: usize,
) -> LlmRealChatResult {
    let mut last = None;
    let mut last_stream_error_with_done = false;
    for attempt in 1..=max_attempts {
        let (events, resp, reasoning) = match chat_stream_once(ctx, &params).await {
            Ok(result) => result,
            Err(err) => {
                eprintln!(
                    "[llm_real] {label} attempt {attempt}/{max_attempts} stream error: {err}"
                );
                if attempt < max_attempts {
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
                panic!("{label} stream failed: {err}");
            }
        };

        let had_stream_error = stream_had_error(&events);
        if had_stream_error {
            eprintln!(
                "[llm_real] {label} attempt {attempt}/{max_attempts} stream error event; last={:?}",
                events.last().map(|e| e.event.as_str())
            );
        }

        let Some(resp) = resp else {
            eprintln!(
                "[llm_real] {label} attempt {attempt}/{max_attempts} missing done payload; events={}",
                events.len()
            );
            if attempt < max_attempts {
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
            panic!("{label} stream missing terminal done payload");
        };

        if had_stream_error && attempt == max_attempts {
            eprintln!(
                "[llm_real] {label} WARNING: final attempt had error event but also produced done payload"
            );
            last_stream_error_with_done = true;
        }

        if had_stream_error && attempt < max_attempts && !ready(&resp) {
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }

        if had_stream_error && attempt < max_attempts && ready(&resp) {
            eprintln!(
                "[llm_real] {label} WARNING: accepting ready response on attempt {attempt} despite stream error event"
            );
        }

        last = Some(LlmRealChatResult {
            resp: resp.clone(),
            reasoning: reasoning.clone(),
            stream_error_with_done: last_stream_error_with_done,
        });

        if ready(&resp) {
            return LlmRealChatResult {
                resp,
                reasoning,
                stream_error_with_done: last_stream_error_with_done,
            };
        }

        eprintln!(
            "[llm_real] {label} attempt {attempt}/{max_attempts} not ready \
             (answer_len={}, degrade={:?}, reasoning_deltas={}); retrying after {}s",
            resp.answer.len(),
            resp.degrade_trace,
            reasoning.delta_count,
            RETRY_DELAY.as_secs()
        );
        if attempt < max_attempts {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    let result = last.unwrap_or_else(|| panic!("{label} stream produced no usable response"));
    if !ready(&result.resp) {
        panic!(
            "{label} stream produced response but readiness check failed on final attempt \
             (answer_len={}, degrade={:?})",
            result.resp.answer.len(),
            result.resp.degrade_trace
        );
    }
    result
}

/// Retry streaming RAG chat until a non-empty answer, capturing reasoning deltas.
pub async fn chat_with_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "rag",
            workspace_id,
            doc_scope,
            session_id: None,
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| !resp.answer.is_empty(),
        "rag chat",
        REAL_LLM_MAX_ATTEMPTS,
    )
    .await
}

fn answer_rejects_synthesis_fallback(answer: &str) -> bool {
    let lower = answer.to_ascii_lowercase();
    !lower.contains("evidence_insufficient_fallback")
        && !lower.contains("could not format a validated cited answer")
        && !lower.contains("could not find relevant evidence in your documents")
}

/// Retry until the answer is non-empty, has citations, and is not a synthesis/evidence fallback.
pub async fn chat_with_citations_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
) -> LlmRealChatResult {
    chat_with_citations_retry_attempts(ctx, query, workspace_id, doc_scope, REAL_LLM_MAX_ATTEMPTS)
        .await
}

pub async fn chat_with_citations_retry_attempts(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
    max_attempts: usize,
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "rag",
            workspace_id,
            doc_scope,
            session_id: None,
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| {
            !resp.answer.is_empty()
                && !resp.citations.is_empty()
                && answer_rejects_synthesis_fallback(&resp.answer)
        },
        "rag chat with citations",
        max_attempts,
    )
    .await
}

/// Retry RAG chat until the response uses multiple tools (non-deterministic probe).
pub async fn chat_with_multitool_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
    min_distinct_tools: usize,
    retrieval_tools: &[&str],
    min_answer_len: usize,
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "rag",
            workspace_id,
            doc_scope,
            session_id: None,
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| {
            if resp.answer.len() < min_answer_len {
                return false;
            }
            let mut names = std::collections::HashSet::new();
            for result in &resp.tool_results {
                names.insert(result.tool.as_str());
            }
            let ready = names.len() >= min_distinct_tools
                && names.iter().any(|tool| retrieval_tools.contains(tool));
            if !ready {
                eprintln!(
                    "[llm_real] rag multitool chat tools={names:?} (need >={min_distinct_tools})"
                );
            }
            ready
        },
        "rag multitool chat",
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS,
    )
    .await
}

/// Retry streaming RAG chat with format_hint until a non-empty answer.
pub async fn chat_with_format_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
    format_hint: &str,
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "rag",
            workspace_id,
            doc_scope,
            session_id: None,
            format_hint: Some(format_hint),
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| !resp.answer.is_empty(),
        "format chat",
        REAL_LLM_MAX_ATTEMPTS,
    )
    .await
}

/// Retry streaming multi-turn RAG chat with an existing session_id.
pub async fn chat_with_session_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
    doc_scope: &[String],
    session_id: &str,
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "rag",
            workspace_id,
            doc_scope,
            session_id: Some(session_id),
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| !resp.answer.is_empty(),
        "session chat",
        REAL_LLM_MAX_ATTEMPTS,
    )
    .await
}

/// Retry streaming general chat until a non-empty answer.
pub async fn chat_general_with_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
) -> LlmRealChatResult {
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "chat",
            workspace_id,
            doc_scope: &[],
            session_id: None,
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| !resp.answer.is_empty(),
        "general chat",
        REAL_LLM_MAX_ATTEMPTS,
    )
    .await
}

/// Retry streaming search until a non-empty, non-degraded answer.
pub async fn search_with_retry(
    ctx: &TestContext,
    query: &str,
    workspace_id: &str,
) -> LlmRealChatResult {
    let empty_scope: &[String] = &[];
    chat_stream_with_retry_inner(
        ctx,
        ChatStreamParams {
            query,
            agent_type: "search",
            workspace_id,
            doc_scope: empty_scope,
            session_id: None,
            format_hint: None,
            debug: true,
            pin_mock_chunk_ids: true,
        },
        |resp| !resp.answer.is_empty() && resp.degrade_trace.is_empty(),
        "search",
        REAL_LLM_MAX_ATTEMPTS,
    )
    .await
}

/// Guard that fails fast if a required real-LLM credential is missing.
pub(crate) fn require_real_llm_config() {
    let required = [
        "AGENT_LLM_BASE_URL",
        "AGENT_LLM_API_KEY",
        "AGENT_LLM_MODEL",
        "EMBEDDING_BASE_URL",
        "EMBEDDING_API_KEY",
        "EMBEDDING_MODEL",
    ];
    for key in &required {
        assert!(
            std::env::var(key).is_ok(),
            "real-LLM test missing required env var: {key}"
        );
    }
}

/// Degrade items that are **not** product failures on happy paths.
///
/// - `doc_scan` / `scan_data`: successful scan path labels (see rag-core doc_scan).
/// - multimodal embedding empty: known soft degrade when no MM content.
pub(crate) fn non_blocking_degrade(
    item: &crate::product_e2e::DegradeTraceItem,
) -> bool {
    use crate::product_e2e::DegradeReason;
    if item.stage == "doc_scan" {
        return matches!(
            &item.reason,
            DegradeReason::Other(msg) if msg == "scan_data" || msg.contains("scan_data")
        ) || item.impact.contains("doc_scan");
    }
    item.stage == "dense_retrieval"
        && matches!(
            &item.reason,
            DegradeReason::Other(msg) if msg.contains("multimodal embedding input is empty")
        )
}

pub mod chat_real;
pub mod write_real;
pub mod format_real;
pub mod multi_turn;
pub mod pdf_corpus;
pub mod pdf_rag_e2e;
pub mod rag_quality_prod;
pub mod rag_real;
pub mod search_real;

#[cfg(test)]
mod stream_reasoning_tests;
mod cost_report;
