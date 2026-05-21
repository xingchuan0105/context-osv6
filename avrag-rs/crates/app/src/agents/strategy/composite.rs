//! CompositeStrategy — cross-mode orchestration that runs RAG + Search in parallel.
//!
//! State machine:
//!   Decompose → ParallelExecute → Merge → Answer
//!
//! The strategy decomposes the user query into RAG-specific and Search-specific
//! sub-queries, executes both branches concurrently, merges the evidence, and
//! synthesizes a unified answer.

use super::{State, StateKind, StepOutcome, Strategy, StrategyContext};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::LoopBudget;
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};
use crate::agents::unified::helpers;
use avrag_llm::{ChatMessage, LlmClient};
use common::{AppError, ToolResult, ToolStatus};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// CompositeState
// ---------------------------------------------------------------------------

/// States in the Composite state machine.
#[derive(Debug)]
pub enum CompositeState {
    /// Decompose: run planner LLM to split query into RAG and Search sub-queries.
    Decompose,
    /// ParallelExecute: run RAG retrieval and web search concurrently.
    ParallelExecute {
        rag_query: String,
        search_query: String,
    },
    /// Merge: combine evidence from both branches.
    Merge {
        rag_tool_results: Vec<ToolResult>,
        search_tool_results: Vec<ToolResult>,
    },
    /// Answer: synthesize unified response.
    Answer {
        merged_context: String,
    },
}

impl State for CompositeState {
    fn state_id(&self) -> &'static str {
        match self {
            CompositeState::Decompose => "decompose",
            CompositeState::ParallelExecute { .. } => "parallel_execute",
            CompositeState::Merge { .. } => "merge",
            CompositeState::Answer { .. } => "answer",
        }
    }

    fn state_kind(&self) -> StateKind {
        match self {
            CompositeState::Decompose => StateKind::Plan,
            CompositeState::ParallelExecute { .. } => StateKind::Execute,
            CompositeState::Merge { .. } => StateKind::Control,
            CompositeState::Answer { .. } => StateKind::Answer,
        }
    }

    fn to_observable(&self) -> serde_json::Value {
        match self {
            CompositeState::Decompose => serde_json::json!({"state": "decompose"}),
            CompositeState::ParallelExecute { rag_query, search_query } => {
                serde_json::json!({
                    "state": "parallel_execute",
                    "rag_query": rag_query,
                    "search_query": search_query,
                })
            }
            CompositeState::Merge { rag_tool_results, search_tool_results } => {
                serde_json::json!({
                    "state": "merge",
                    "rag_result_count": rag_tool_results.len(),
                    "search_result_count": search_tool_results.len(),
                })
            }
            CompositeState::Answer { .. } => serde_json::json!({"state": "answer"}),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// CompositeContext
// ---------------------------------------------------------------------------

/// Runtime context for CompositeStrategy.
pub struct CompositeContext {
    pub request: AgentRequest,
    pub trace_id: String,
    pub budget: LoopBudget,
    pub sink: Box<dyn AgentEventSink>,
    pub cancel: CancellationToken,
    pub auth: avrag_auth::AuthContext,

    // Accumulated
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    pub request_count: u64,
    pub all_tool_results: Vec<ToolResult>,
}

impl StrategyContext for CompositeContext {
    fn trace_id(&self) -> &str {
        &self.trace_id
    }

    fn budget(&self) -> &LoopBudget {
        &self.budget
    }

    fn budget_mut(&mut self) -> &mut LoopBudget {
        &mut self.budget
    }

    fn sink(&self) -> &dyn AgentEventSink {
        self.sink.as_ref()
    }

    fn cancel(&self) -> &CancellationToken {
        &self.cancel
    }

    fn org_id(&self) -> Option<String> {
        Some(self.auth.org_id().to_string())
    }

    fn actor_id(&self) -> Option<String> {
        self.auth.actor_id().map(|id| id.uuid().to_string())
    }

    fn request(&self) -> Option<&crate::agents::runtime::AgentRequest> {
        Some(&self.request)
    }
}

impl CompositeContext {
    /// Build a CompositeContext from an AgentRequest.
    pub fn from_request(
        request: AgentRequest,
        trace_id: String,
        budget: LoopBudget,
        sink: Box<dyn AgentEventSink>,
        cancel: CancellationToken,
    ) -> Result<Self, AppError> {
        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone()).map_err(|error| {
                AppError::internal(format!("Failed to deserialize auth context: {error}"))
            })?;
        Ok(Self {
            request,
            trace_id,
            budget,
            sink,
            cancel,
            auth,
            aggregated_usage: None,
            request_count: 0,
            all_tool_results: Vec::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// CompositeStrategy
// ---------------------------------------------------------------------------

/// Strategy implementation for cross-mode composite execution.
pub struct CompositeStrategy {
    pub llm: LlmClient,
    pub temperature: Option<f32>,
    pub rag_runtime: Option<std::sync::Arc<avrag_rag_core::RagRuntime>>,
    pub search_executor: Option<std::sync::Arc<dyn avrag_search::SearchProvider>>,
}

#[async_trait::async_trait]
impl Strategy for CompositeStrategy {
    type Context = CompositeContext;

    async fn init(
        &self,
        _ctx: &mut CompositeContext,
    ) -> Result<Box<dyn State>, AppError> {
        Ok(Box::new(CompositeState::Decompose))
    }

    async fn step(
        &self,
        state: Box<dyn State>,
        ctx: &mut CompositeContext,
    ) -> Result<StepOutcome, AppError> {
        let composite_state = state
            .as_any()
            .downcast_ref::<CompositeState>()
            .ok_or_else(|| AppError::internal("invalid state type for CompositeStrategy"))?;

        match composite_state {
            CompositeState::Decompose => self.step_decompose(ctx).await,
            CompositeState::ParallelExecute { rag_query, search_query } => {
                self.step_parallel_execute(ctx, rag_query.clone(), search_query.clone()).await
            }
            CompositeState::Merge { rag_tool_results, search_tool_results } => {
                self.step_merge(ctx, rag_tool_results.clone(), search_tool_results.clone()).await
            }
            CompositeState::Answer { merged_context } => {
                self.step_answer(ctx, merged_context.clone()).await
            }
        }
    }
}

impl CompositeStrategy {
    // --- Decompose step ---

    async fn step_decompose(&self, ctx: &mut CompositeContext) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;
        ctx.budget.tick();

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "composite".to_string(),
                message: "Decomposing query for parallel RAG + Search".to_string(),
            })
            .await;

        let system_prompt = concat!(
            "You are a query decomposition expert. Given a user query, decide how to split it ",
            "into a RAG (retrieval from internal documents) sub-query and a Search ",
            "(web search) sub-query. Respond in JSON:\n",
            "{\"rag_query\": string, \"search_query\": string, \"reasoning\": string}\n",
            "If the query only needs one mode, set the other to empty string."
        );

        let messages = vec![
            ChatMessage::system(system_prompt.to_string()),
            ChatMessage::user(format!("Query: {}", ctx.request.query)),
        ];

        let response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(AppError::internal("request cancelled"));
            }
            result = self.llm.complete(&messages, self.temperature) => {
                result.map_err(|e| AppError::internal(format!("Composite decompose failed: {e}")))?
            }
        };

        ctx.request_count += 1;
        ctx.aggregated_usage = Some(helpers::merge_usage(
            ctx.aggregated_usage.as_ref(),
            &response.usage,
        ));

        let decision = parse_decompose_decision(&response.content);

        let rag_query = if decision.rag_query.is_empty() {
            ctx.request.query.clone()
        } else {
            decision.rag_query
        };

        let search_query = if decision.search_query.is_empty() {
            ctx.request.query.clone()
        } else {
            decision.search_query
        };

        // If no RAG runtime, skip RAG; if no search executor, skip search.
        let has_rag = self.rag_runtime.is_some() && !ctx.request.doc_scope.is_empty();
        let has_search = self.search_executor.is_some();

        if !has_rag && !has_search {
            return Ok(StepOutcome::Terminate(AgentRunResult {
                answer: "No retrieval or search backend is configured.".to_string(),
                final_decision: Some(FinalDecision::Synthesized),
                ..Default::default()
            }));
        }

        let final_rag_query = if has_rag { rag_query } else { String::new() };
        let final_search_query = if has_search { search_query } else { String::new() };

        Ok(StepOutcome::Next(Box::new(CompositeState::ParallelExecute {
            rag_query: final_rag_query,
            search_query: final_search_query,
        })))
    }

    // --- ParallelExecute step ---

    async fn step_parallel_execute(
        &self,
        ctx: &mut CompositeContext,
        rag_query: String,
        search_query: String,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "composite".to_string(),
                message: "Executing RAG + Search in parallel".to_string(),
            })
            .await;

        // Spawn RAG task
        let rag_task: Option<tokio::task::JoinHandle<Vec<ToolResult>>> = if !rag_query.is_empty() {
            let runtime = self.rag_runtime.clone().unwrap();
            let query = rag_query.clone();
            let doc_scope = ctx.request.doc_scope.clone();
            let auth = ctx.auth.clone();
            let cancel = ctx.cancel.clone();
            Some(tokio::spawn(async move {
                run_rag_retrieval(&runtime, &query, doc_scope, &auth, &cancel).await
            }))
        } else {
            None
        };

        // Spawn Search task
        let search_task: Option<tokio::task::JoinHandle<Vec<ToolResult>>> =
            if !search_query.is_empty() {
                let executor = self.search_executor.clone().unwrap();
                let query = search_query.clone();
                let auth = ctx.auth.clone();
                let cancel = ctx.cancel.clone();
                Some(tokio::spawn(async move {
                    run_web_search(executor.as_ref(), &query, &auth, &cancel).await
                }))
            } else {
                None
            };

        // Await both
        let rag_results = match rag_task {
            Some(t) => match t.await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "RAG task panicked");
                    vec![ToolResult {
                        tool: "rag_retrieval".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::Error,
                        data: Some(serde_json::json!({"error": format!("RAG task failed: {e}")})),
                        trace: None,
                    }]
                }
            },
            None => Vec::new(),
        };

        let search_results = match search_task {
            Some(t) => match t.await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "Search task panicked");
                    vec![ToolResult {
                        tool: "web_search".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::Error,
                        data: Some(serde_json::json!({"error": format!("Search task failed: {e}")})),
                        trace: None,
                    }]
                }
            },
            None => Vec::new(),
        };

        ctx.all_tool_results.extend(rag_results.iter().cloned());
        ctx.all_tool_results.extend(search_results.iter().cloned());

        Ok(StepOutcome::Next(Box::new(CompositeState::Merge {
            rag_tool_results: rag_results,
            search_tool_results: search_results,
        })))
    }

    // --- Merge step ---

    async fn step_merge(
        &self,
        ctx: &mut CompositeContext,
        rag_tool_results: Vec<ToolResult>,
        search_tool_results: Vec<ToolResult>,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let mut context_parts = Vec::new();

        // Extract RAG chunks
        let rag_chunks = helpers::extract_chunks_with_scores(&rag_tool_results);
        if !rag_chunks.is_empty() {
            let mut rag_context = String::from("## Internal Documents (RAG)\n\n");
            for (chunk, score) in &rag_chunks {
                rag_context.push_str(&format!(
                    "- [{}] (score={:.3}): {}\n",
                    chunk.chunk_id,
                    score,
                    chunk.text
                ));
            }
            context_parts.push(rag_context);
        }

        // Extract Search results
        let search_items = extract_search_results(&search_tool_results);
        if !search_items.is_empty() {
            let mut search_context = String::from("## Web Search Results\n\n");
            for item in &search_items {
                search_context.push_str(&format!(
                    "- [{}]({}): {}\n",
                    item.title, item.url, item.snippet
                ));
            }
            context_parts.push(search_context);
        }

        if context_parts.is_empty() {
            context_parts.push("No relevant information found from either RAG or Search.".to_string());
        }

        let merged_context = context_parts.join("\n\n---\n\n");

        Ok(StepOutcome::Next(Box::new(CompositeState::Answer {
            merged_context,
        })))
    }

    // --- Answer step ---

    async fn step_answer(
        &self,
        ctx: &mut CompositeContext,
        merged_context: String,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let system_prompt = concat!(
            "You are a helpful assistant. Synthesize a clear, accurate answer using the ",
            "provided context from both internal documents (RAG) and web search results. ",
            "Cite sources when possible. If the sources conflict, note the discrepancy."
        );

        let mut messages = vec![
            ChatMessage::system(system_prompt.to_string()),
            ChatMessage::user(format!("Question: {}\n\nContext:\n{}", ctx.request.query, merged_context)),
        ];

        // Inject session summary if present
        if let Some(summary) = ctx.request.session_summary.as_deref().filter(|s| !s.trim().is_empty()) {
            messages.insert(1, ChatMessage::user(format!("Session summary: {summary}")));
        }

        let sink = ctx.sink.as_ref();
        let cancel = ctx.cancel.clone();

        let response = if ctx.request.stream {
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let stream = self.llm.complete_stream(&messages, self.temperature, cancel, move |delta| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            });
            tokio::pin!(stream);

            let response = loop {
                tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => {
                        return Err(AppError::internal("request cancelled"));
                    }
                    delta = delta_rx.recv() => {
                        if let Some(delta) = delta {
                            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                        }
                    }
                    result = &mut stream => {
                        break result.map_err(|e| AppError::internal(format!("LLM stream failed: {e}")))?;
                    }
                }
            };

            while let Ok(delta) = delta_rx.try_recv() {
                let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
            }

            response
        } else {
            let response = tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => {
                    return Err(AppError::internal("request cancelled"));
                }
                result = self.llm.complete(&messages, self.temperature) => {
                    result.map_err(|e| AppError::internal(format!("LLM completion failed: {e}")))?
                }
            };
            let _ = sink.emit(AgentEvent::MessageDelta { text: response.content.clone() }).await;
            response
        };

        ctx.request_count += 1;
        ctx.aggregated_usage = Some(helpers::merge_usage(
            ctx.aggregated_usage.as_ref(),
            &response.usage,
        ));

        let run_usage = helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count);

        let _ = helpers::emit_usage(sink, run_usage.as_ref()).await;
        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(response.content.clone()),
                usage: run_usage.as_ref().map(helpers::run_usage_to_agent_usage),
            })
            .await;

        let result = AgentRunResult {
            answer: response.content,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.all_tool_results),
            final_decision: Some(FinalDecision::Synthesized),
            ..Default::default()
        };

        Ok(StepOutcome::Terminate(result))
    }
}

// ---------------------------------------------------------------------------
// Decompose decision
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct DecomposeDecision {
    rag_query: String,
    search_query: String,
    #[allow(dead_code)]
    reasoning: String,
}

fn parse_decompose_decision(raw: &str) -> DecomposeDecision {
    let start = raw.find('{');
    let end = raw.rfind('}');
    let json_str = match (start, end) {
        (Some(s), Some(e)) if s <= e => &raw[s..=e],
        _ => raw.trim(),
    };

    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return DecomposeDecision::default(),
    };

    DecomposeDecision {
        rag_query: value
            .get("rag_query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        search_query: value
            .get("search_query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        reasoning: value
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Parallel execution helpers
// ---------------------------------------------------------------------------

async fn run_rag_retrieval(
    runtime: &avrag_rag_core::RagRuntime,
    query: &str,
    doc_scope: Vec<String>,
    auth: &avrag_auth::AuthContext,
    cancel: &CancellationToken,
) -> Vec<ToolResult> {
    let chat_req = common::ChatRequest {
        query: query.to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: "rag".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: doc_scope.to_vec(),
        messages: vec![],
        stream: false,
        language: None,
    };

    // Use the runtime's planner to get a plan
    let mut degrade_trace = Vec::new();
    let plan_result = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return vec![ToolResult {
                tool: "rag_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "cancelled"})),
                trace: None,
            }];
        }
        result = runtime.plan(&chat_req, None, None, &mut degrade_trace) => result,
    };

    let (rag_plan, _planner_usage) = match plan_result {
        Ok(p) => p,
        Err(e) => {
            return vec![ToolResult {
                tool: "rag_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": format!("Planner failed: {e}")})),
                trace: None,
            }];
        }
    };

    // Run dense retrieval
    let retrieval_result = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return vec![ToolResult {
                tool: "rag_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "cancelled"})),
                trace: None,
            }];
        }
        result = runtime.retrieve_text_dense_stage(&chat_req, auth, &rag_plan) => result,
    };

    match retrieval_result {
        Ok((weighted_lists, _degrade)) => {
            let mut all_chunks = Vec::new();
            for list in &weighted_lists {
                for chunk in &list.chunks {
                    all_chunks.push(serde_json::json!({
                        "chunk_id": chunk.chunk_id.to_string(),
                        "doc_id": chunk.doc_id.to_string(),
                        "text": chunk.content,
                        "page": chunk.page,
                        "score": chunk.score,
                    }));
                }
            }
            vec![ToolResult {
                tool: "rag_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::Value::Array(all_chunks)),
                trace: None,
            }]
        }
        Err(e) => {
            vec![ToolResult {
                tool: "rag_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": format!("Retrieval failed: {e}")})),
                trace: None,
            }]
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SearchItem {
    title: String,
    url: String,
    snippet: String,
}

async fn run_web_search(
    executor: &dyn avrag_search::SearchProvider,
    query: &str,
    _auth: &avrag_auth::AuthContext,
    cancel: &CancellationToken,
) -> Vec<ToolResult> {
    let search_result = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            return vec![ToolResult {
                tool: "web_search".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "cancelled"})),
                trace: None,
            }];
        }
        result = executor.execute_search(query, None) => result,
    };

    match search_result {
        Ok(response) => {
            let items: Vec<SearchItem> = response
                .results
                .iter()
                .map(|r| SearchItem {
                    title: r.title.clone(),
                    url: r.url.clone(),
                    snippet: r.snippet.clone(),
                })
                .collect();
            vec![ToolResult {
                tool: "web_search".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::to_value(&items).unwrap_or_default()),
                trace: None,
            }]
        }
        Err(e) => {
            vec![ToolResult {
                tool: "web_search".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": format!("Search failed: {e}")})),
                trace: None,
            }]
        }
    }
}

fn extract_search_results(tool_results: &[ToolResult]) -> Vec<SearchItem> {
    let mut out = Vec::new();
    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        if let Some(data) = &result.data
            && let Ok(items) = serde_json::from_value::<Vec<SearchItem>>(data.clone()) {
                out.extend(items);
            }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_state_ids() {
        assert_eq!(CompositeState::Decompose.state_id(), "decompose");
        assert_eq!(
            CompositeState::ParallelExecute {
                rag_query: "a".to_string(),
                search_query: "b".to_string(),
            }
            .state_id(),
            "parallel_execute"
        );
        assert_eq!(
            CompositeState::Merge {
                rag_tool_results: vec![],
                search_tool_results: vec![],
            }
            .state_id(),
            "merge"
        );
        assert_eq!(
            CompositeState::Answer {
                merged_context: "ctx".to_string(),
            }
            .state_id(),
            "answer"
        );
    }

    #[test]
    fn composite_state_kinds() {
        assert_eq!(CompositeState::Decompose.state_kind(), StateKind::Plan);
        assert_eq!(
            CompositeState::ParallelExecute {
                rag_query: "a".to_string(),
                search_query: "b".to_string(),
            }
            .state_kind(),
            StateKind::Execute
        );
        assert_eq!(
            CompositeState::Merge {
                rag_tool_results: vec![],
                search_tool_results: vec![],
            }
            .state_kind(),
            StateKind::Control
        );
        assert_eq!(
            CompositeState::Answer {
                merged_context: "ctx".to_string(),
            }
            .state_kind(),
            StateKind::Answer
        );
    }

    #[test]
    fn parse_decompose_decision_valid() {
        let raw = r#"{"rag_query": "company revenue 2024", "search_query": "market trends 2024", "reasoning": "split"}"#;
        let d = parse_decompose_decision(raw);
        assert_eq!(d.rag_query, "company revenue 2024");
        assert_eq!(d.search_query, "market trends 2024");
        assert_eq!(d.reasoning, "split");
    }

    #[test]
    fn parse_decompose_decision_invalid_defaults_empty() {
        let raw = "not json";
        let d = parse_decompose_decision(raw);
        assert!(d.rag_query.is_empty());
        assert!(d.search_query.is_empty());
    }

    #[test]
    fn extract_search_results_ok() {
        let items = vec![SearchItem {
            title: "T".to_string(),
            url: "http://x".to_string(),
            snippet: "S".to_string(),
        }];
        let tool_results = vec![ToolResult {
            tool: "web_search".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::to_value(&items).unwrap()),
            trace: None,
        }];
        let extracted = extract_search_results(&tool_results);
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].title, "T");
    }

    #[test]
    fn extract_search_results_skips_errors() {
        let tool_results = vec![ToolResult {
            tool: "web_search".to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Error,
            data: None,
            trace: None,
        }];
        let extracted = extract_search_results(&tool_results);
        assert!(extracted.is_empty());
    }
}
