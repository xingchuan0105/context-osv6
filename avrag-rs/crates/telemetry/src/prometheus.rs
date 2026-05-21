use prometheus_client::encoding::{EncodeLabelSet, text::encode};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;
use std::sync::LazyLock;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpLabels {
    pub route: String,
    pub method: String,
    pub status_class: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct RouteMethodLabels {
    pub route: String,
    pub method: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct SingleLabel {
    pub value: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct SurfaceEventLabels {
    pub surface: String,
    pub event_type: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TaskKindLabels {
    pub task_kind: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TaskResultLabels {
    pub task_kind: String,
    pub result: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct LlmLabels {
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub result: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct LlmDurationLabels {
    pub feature: String,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct LlmUsageLabels {
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub token_type: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct RetrievalLabels {
    pub mode: String,
    pub stage: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ModeLabel {
    pub mode: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct GuardrailLabels {
    pub guard_type: String,
    pub action: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct DegradeLabels {
    pub agent_type: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Agent v5 metrics labels
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct AgentRunLabels {
    pub strategy: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct AgentStateLabels {
    pub strategy: String,
    pub state_id: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct AgentToolLabels {
    pub tool_name: String,
    pub status: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct AgentErrorLabels {
    pub error_kind: String,
}

struct MetricsState {
    registry: Registry,
    http_requests_total: Family<HttpLabels, Counter>,
    http_request_duration_ms: Family<RouteMethodLabels, Histogram>,
    http_inflight_requests: Family<SingleLabel, Gauge>,
    sse_streams_open: Family<SingleLabel, Gauge>,
    sse_events_sent_total: Family<SurfaceEventLabels, Counter>,
    upload_requests_total: Family<SingleLabel, Counter>,
    upload_bytes_total: Family<SingleLabel, Counter>,
    worker_tasks_started_total: Family<TaskKindLabels, Counter>,
    worker_tasks_completed_total: Family<TaskResultLabels, Counter>,
    worker_task_duration_ms: Family<TaskKindLabels, Histogram>,
    llm_calls_total: Family<LlmLabels, Counter>,
    llm_call_duration_ms: Family<LlmDurationLabels, Histogram>,
    llm_usage_tokens_total: Family<LlmUsageLabels, Counter>,
    retrieval_requests_total: Family<RetrievalLabels, Counter>,
    retrieval_zero_result_total: Family<ModeLabel, Counter>,
    guardrail_blocks_total: Family<GuardrailLabels, Counter>,
    usage_limit_blocks_total: Family<SingleLabel, Counter>,
    dependency_failures_total: Family<SingleLabel, Counter>,
    degrades_total: Family<DegradeLabels, Counter>,
    // v5 agent metrics
    agent_run_total: Family<AgentRunLabels, Counter>,
    agent_run_duration_ms: Family<AgentRunLabels, Histogram>,
    agent_state_duration_ms: Family<AgentStateLabels, Histogram>,
    agent_tool_call_total: Family<AgentToolLabels, Counter>,
    agent_tool_call_duration_ms: Family<AgentToolLabels, Histogram>,
    agent_budget_exhausted_total: Family<AgentRunLabels, Counter>,
    agent_error_total: Family<AgentErrorLabels, Counter>,
}

impl MetricsState {
    fn new() -> Self {
        let mut registry = Registry::default();
        let http_requests_total = Family::<HttpLabels, Counter>::default();
        let http_request_duration_ms =
            Family::<RouteMethodLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(1.0, 2.0, 16))
            });
        let http_inflight_requests = Family::<SingleLabel, Gauge>::default();
        let sse_streams_open = Family::<SingleLabel, Gauge>::default();
        let sse_events_sent_total = Family::<SurfaceEventLabels, Counter>::default();
        let upload_requests_total = Family::<SingleLabel, Counter>::default();
        let upload_bytes_total = Family::<SingleLabel, Counter>::default();
        let worker_tasks_started_total = Family::<TaskKindLabels, Counter>::default();
        let worker_tasks_completed_total = Family::<TaskResultLabels, Counter>::default();
        let worker_task_duration_ms =
            Family::<TaskKindLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(1.0, 2.0, 18))
            });
        let llm_calls_total = Family::<LlmLabels, Counter>::default();
        let llm_call_duration_ms =
            Family::<LlmDurationLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(10.0, 2.0, 16))
            });
        let llm_usage_tokens_total = Family::<LlmUsageLabels, Counter>::default();
        let retrieval_requests_total = Family::<RetrievalLabels, Counter>::default();
        let retrieval_zero_result_total = Family::<ModeLabel, Counter>::default();
        let guardrail_blocks_total = Family::<GuardrailLabels, Counter>::default();
        let usage_limit_blocks_total = Family::<SingleLabel, Counter>::default();
        let dependency_failures_total = Family::<SingleLabel, Counter>::default();
        let degrades_total = Family::<DegradeLabels, Counter>::default();
        let agent_run_total = Family::<AgentRunLabels, Counter>::default();
        let agent_run_duration_ms =
            Family::<AgentRunLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(10.0, 2.0, 16))
            });
        let agent_state_duration_ms =
            Family::<AgentStateLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(1.0, 2.0, 16))
            });
        let agent_tool_call_total = Family::<AgentToolLabels, Counter>::default();
        let agent_tool_call_duration_ms =
            Family::<AgentToolLabels, Histogram>::new_with_constructor(|| {
                Histogram::new(exponential_buckets(1.0, 2.0, 16))
            });
        let agent_budget_exhausted_total = Family::<AgentRunLabels, Counter>::default();
        let agent_error_total = Family::<AgentErrorLabels, Counter>::default();

        registry.register(
            "http_requests_total",
            "Total HTTP requests by route, method, and status class.",
            http_requests_total.clone(),
        );
        registry.register(
            "http_request_duration_ms",
            "HTTP request duration in milliseconds by route and method.",
            http_request_duration_ms.clone(),
        );
        registry.register(
            "http_inflight_requests",
            "Inflight HTTP requests by normalized route.",
            http_inflight_requests.clone(),
        );
        registry.register(
            "sse_streams_open",
            "Open SSE streams by surface.",
            sse_streams_open.clone(),
        );
        registry.register(
            "sse_events_sent_total",
            "Total SSE events sent by surface and event type.",
            sse_events_sent_total.clone(),
        );
        registry.register(
            "upload_requests_total",
            "Upload requests by upload kind.",
            upload_requests_total.clone(),
        );
        registry.register(
            "upload_bytes_total",
            "Uploaded bytes by upload kind.",
            upload_bytes_total.clone(),
        );
        registry.register(
            "worker_tasks_started_total",
            "Worker tasks started by task kind.",
            worker_tasks_started_total.clone(),
        );
        registry.register(
            "worker_tasks_completed_total",
            "Worker tasks completed by task kind and result.",
            worker_tasks_completed_total.clone(),
        );
        registry.register(
            "worker_task_duration_ms",
            "Worker task duration in milliseconds by task kind.",
            worker_task_duration_ms.clone(),
        );
        registry.register(
            "llm_calls_total",
            "LLM calls by feature, provider, model, and result.",
            llm_calls_total.clone(),
        );
        registry.register(
            "llm_call_duration_ms",
            "LLM call duration in milliseconds by feature, provider, and model.",
            llm_call_duration_ms.clone(),
        );
        registry.register(
            "llm_usage_tokens_total",
            "LLM token usage by feature, provider, model, and token type.",
            llm_usage_tokens_total.clone(),
        );
        registry.register(
            "retrieval_requests_total",
            "Retrieval requests by mode and stage.",
            retrieval_requests_total.clone(),
        );
        registry.register(
            "retrieval_zero_result_total",
            "Retrieval requests that returned zero results by mode.",
            retrieval_zero_result_total.clone(),
        );
        registry.register(
            "guardrail_blocks_total",
            "Guardrail blocks by guard type and action.",
            guardrail_blocks_total.clone(),
        );
        registry.register(
            "usage_limit_blocks_total",
            "Usage-limit blocks by rolling window.",
            usage_limit_blocks_total.clone(),
        );
        registry.register(
            "dependency_failures_total",
            "Dependency failures by dependency name.",
            dependency_failures_total.clone(),
        );
        registry.register(
            "degrades_total",
            "Agent degrade events by agent type and reason.",
            degrades_total.clone(),
        );
        registry.register(
            "agent_run_total",
            "Total agent runs by strategy.",
            agent_run_total.clone(),
        );
        registry.register(
            "agent_run_duration_ms",
            "Agent run duration in milliseconds by strategy.",
            agent_run_duration_ms.clone(),
        );
        registry.register(
            "agent_state_duration_ms",
            "Agent state duration in milliseconds by strategy and state_id.",
            agent_state_duration_ms.clone(),
        );
        registry.register(
            "agent_tool_call_total",
            "Total agent tool calls by tool_name and status.",
            agent_tool_call_total.clone(),
        );
        registry.register(
            "agent_tool_call_duration_ms",
            "Agent tool call duration in milliseconds by tool_name.",
            agent_tool_call_duration_ms.clone(),
        );
        registry.register(
            "agent_budget_exhausted_total",
            "Total agent budget exhausted events by strategy.",
            agent_budget_exhausted_total.clone(),
        );
        registry.register(
            "agent_error_total",
            "Total agent errors by error_kind.",
            agent_error_total.clone(),
        );

        Self {
            registry,
            http_requests_total,
            http_request_duration_ms,
            http_inflight_requests,
            sse_streams_open,
            sse_events_sent_total,
            upload_requests_total,
            upload_bytes_total,
            worker_tasks_started_total,
            worker_tasks_completed_total,
            worker_task_duration_ms,
            llm_calls_total,
            llm_call_duration_ms,
            llm_usage_tokens_total,
            retrieval_requests_total,
            retrieval_zero_result_total,
            guardrail_blocks_total,
            usage_limit_blocks_total,
            dependency_failures_total,
            degrades_total,
            agent_run_total,
            agent_run_duration_ms,
            agent_state_duration_ms,
            agent_tool_call_total,
            agent_tool_call_duration_ms,
            agent_budget_exhausted_total,
            agent_error_total,
        }
    }
}

static METRICS: LazyLock<MetricsState> = LazyLock::new(MetricsState::new);

pub fn encode_metrics() -> String {
    let mut out = String::new();
    encode(&mut out, &METRICS.registry).expect("metrics encode should succeed");
    out
}

pub fn inc_http_inflight(route: &str) {
    METRICS
        .http_inflight_requests
        .get_or_create(&SingleLabel {
            value: route.to_string(),
        })
        .inc();
}

pub fn dec_http_inflight(route: &str) {
    METRICS
        .http_inflight_requests
        .get_or_create(&SingleLabel {
            value: route.to_string(),
        })
        .dec();
}

pub fn observe_http_request(route: &str, method: &str, status_code: u16, duration_ms: f64) {
    METRICS
        .http_requests_total
        .get_or_create(&HttpLabels {
            route: route.to_string(),
            method: normalize_method(method),
            status_class: status_class(status_code),
        })
        .inc();
    METRICS
        .http_request_duration_ms
        .get_or_create(&RouteMethodLabels {
            route: route.to_string(),
            method: normalize_method(method),
        })
        .observe(duration_ms);
}

pub fn inc_sse_streams(surface: &str) {
    METRICS
        .sse_streams_open
        .get_or_create(&SingleLabel {
            value: surface.to_string(),
        })
        .inc();
}

pub fn dec_sse_streams(surface: &str) {
    METRICS
        .sse_streams_open
        .get_or_create(&SingleLabel {
            value: surface.to_string(),
        })
        .dec();
}

pub fn observe_sse_event(surface: &str, event_type: &str) {
    METRICS
        .sse_events_sent_total
        .get_or_create(&SurfaceEventLabels {
            surface: surface.to_string(),
            event_type: event_type.to_string(),
        })
        .inc();
}

pub fn observe_upload(kind: &str, bytes: u64) {
    METRICS
        .upload_requests_total
        .get_or_create(&SingleLabel {
            value: kind.to_string(),
        })
        .inc();
    METRICS
        .upload_bytes_total
        .get_or_create(&SingleLabel {
            value: kind.to_string(),
        })
        .inc_by(bytes);
}

pub fn observe_worker_task_started(task_kind: &str) {
    METRICS
        .worker_tasks_started_total
        .get_or_create(&TaskKindLabels {
            task_kind: task_kind.to_string(),
        })
        .inc();
}

pub fn observe_worker_task_completed(task_kind: &str, result: &str, duration_ms: f64) {
    METRICS
        .worker_tasks_completed_total
        .get_or_create(&TaskResultLabels {
            task_kind: task_kind.to_string(),
            result: result.to_string(),
        })
        .inc();
    METRICS
        .worker_task_duration_ms
        .get_or_create(&TaskKindLabels {
            task_kind: task_kind.to_string(),
        })
        .observe(duration_ms);
}

pub fn observe_llm_call(
    feature: &str,
    provider: &str,
    model: &str,
    result: &str,
    duration_ms: f64,
) {
    METRICS
        .llm_calls_total
        .get_or_create(&LlmLabels {
            feature: non_empty(feature),
            provider: non_empty(provider),
            model: non_empty(model),
            result: non_empty(result),
        })
        .inc();
    METRICS
        .llm_call_duration_ms
        .get_or_create(&LlmDurationLabels {
            feature: non_empty(feature),
            provider: non_empty(provider),
            model: non_empty(model),
        })
        .observe(duration_ms);
}

pub fn observe_llm_usage(
    feature: &str,
    provider: &str,
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
) {
    let total = prompt_tokens + completion_tokens;
    METRICS
        .llm_usage_tokens_total
        .get_or_create(&LlmUsageLabels {
            feature: non_empty(feature),
            provider: non_empty(provider),
            model: non_empty(model),
            token_type: "prompt".to_string(),
        })
        .inc_by(prompt_tokens);
    METRICS
        .llm_usage_tokens_total
        .get_or_create(&LlmUsageLabels {
            feature: non_empty(feature),
            provider: non_empty(provider),
            model: non_empty(model),
            token_type: "completion".to_string(),
        })
        .inc_by(completion_tokens);
    METRICS
        .llm_usage_tokens_total
        .get_or_create(&LlmUsageLabels {
            feature: non_empty(feature),
            provider: non_empty(provider),
            model: non_empty(model),
            token_type: "total".to_string(),
        })
        .inc_by(total);
}

pub fn observe_retrieval_request(mode: &str, stage: &str) {
    METRICS
        .retrieval_requests_total
        .get_or_create(&RetrievalLabels {
            mode: non_empty(mode),
            stage: non_empty(stage),
        })
        .inc();
}

pub fn observe_retrieval_zero_result(mode: &str) {
    METRICS
        .retrieval_zero_result_total
        .get_or_create(&ModeLabel {
            mode: non_empty(mode),
        })
        .inc();
}

pub fn observe_guardrail_block(guard_type: &str, action: &str) {
    METRICS
        .guardrail_blocks_total
        .get_or_create(&GuardrailLabels {
            guard_type: non_empty(guard_type),
            action: non_empty(action),
        })
        .inc();
}

pub fn observe_usage_limit_block(window: &str) {
    METRICS
        .usage_limit_blocks_total
        .get_or_create(&SingleLabel {
            value: non_empty(window),
        })
        .inc();
}

pub fn record_dependency_failure(name: &str) {
    METRICS
        .dependency_failures_total
        .get_or_create(&SingleLabel {
            value: non_empty(name),
        })
        .inc();
}

pub fn observe_degrade(agent_type: &str, reason: &str) {
    METRICS
        .degrades_total
        .get_or_create(&DegradeLabels {
            agent_type: non_empty(agent_type),
            reason: non_empty(reason),
        })
        .inc();
}

// ---------------------------------------------------------------------------
// Agent v5 metrics
// ---------------------------------------------------------------------------

pub fn observe_agent_run(strategy: &str, duration_ms: f64) {
    METRICS
        .agent_run_total
        .get_or_create(&AgentRunLabels {
            strategy: non_empty(strategy),
        })
        .inc();
    METRICS
        .agent_run_duration_ms
        .get_or_create(&AgentRunLabels {
            strategy: non_empty(strategy),
        })
        .observe(duration_ms);
}

pub fn observe_agent_state(strategy: &str, state_id: &str, duration_ms: f64) {
    METRICS
        .agent_state_duration_ms
        .get_or_create(&AgentStateLabels {
            strategy: non_empty(strategy),
            state_id: non_empty(state_id),
        })
        .observe(duration_ms);
}

pub fn observe_agent_tool_call(tool_name: &str, status: &str, duration_ms: f64) {
    METRICS
        .agent_tool_call_total
        .get_or_create(&AgentToolLabels {
            tool_name: non_empty(tool_name),
            status: non_empty(status),
        })
        .inc();
    METRICS
        .agent_tool_call_duration_ms
        .get_or_create(&AgentToolLabels {
            tool_name: non_empty(tool_name),
            status: "".to_string(),
        })
        .observe(duration_ms);
}

pub fn observe_agent_budget_exhausted(strategy: &str) {
    METRICS
        .agent_budget_exhausted_total
        .get_or_create(&AgentRunLabels {
            strategy: non_empty(strategy),
        })
        .inc();
}

pub fn observe_agent_error(error_kind: &str) {
    METRICS
        .agent_error_total
        .get_or_create(&AgentErrorLabels {
            error_kind: non_empty(error_kind),
        })
        .inc();
}

fn non_empty(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

fn normalize_method(method: &str) -> String {
    match method {
        "GET" => "get".to_string(),
        "POST" => "post".to_string(),
        "PUT" => "put".to_string(),
        "PATCH" => "patch".to_string(),
        "DELETE" => "delete".to_string(),
        _ => "other".to_string(),
    }
}

fn status_class(status_code: u16) -> String {
    match status_code {
        200..=299 => "2xx".to_string(),
        300..=399 => "3xx".to_string(),
        400..=499 => "4xx".to_string(),
        500..=599 => "5xx".to_string(),
        _ => "other".to_string(),
    }
}
