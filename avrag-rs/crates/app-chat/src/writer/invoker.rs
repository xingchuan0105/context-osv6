//! Subagent invoker: run existing react-loop workers for write-mode research.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use avrag_guardrails::GuardPipeline;
use tokio::time::timeout;

use agent_loop::runtime::{AgentRequest, AgentRunResult};
use crate::agents::service::UnifiedAgentService;
use crate::agents::AgentKind;
use agent_loop::events::CollectingSink;

const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(120);

/// Runs RAG / Search react loops as research sub-workers (spec §6.2).
pub struct SubagentInvoker {
    service: Arc<UnifiedAgentService>,
    guard: Option<Arc<GuardPipeline>>,
}

impl SubagentInvoker {
    pub fn new(service: Arc<UnifiedAgentService>, guard: Option<Arc<GuardPipeline>>) -> Self {
        Self { service, guard }
    }

    /// Build a worker [`AgentRequest`] from a parent request template.
    pub fn worker_request(parent: &AgentRequest, kind: AgentKind, query: &str) -> AgentRequest {
        AgentRequest {
            kind,
            query: query.to_string(),
            workspace_id: parent.workspace_id.clone(),
            session_id: parent.session_id.clone(),
            doc_scope: parent.doc_scope.clone(),
            messages: Vec::new(),
            user_preferences: None,
            debug: parent.debug,
            stream: false,
            language: parent.language.clone(),
            preferred_tools: Vec::new(),
            format_hint: None,
            max_iterations: Some(3),
            auth: parent.auth.clone(),
            docscope_metadata: parent.docscope_metadata.clone(),
            metadata: parent.metadata.clone(),
            cancellation_token: parent.cancellation_token.clone(),
            guard_pipeline: parent.guard_pipeline.clone(),
        }
    }

    /// Run one worker under a collecting sink with timeout and token budget caps.
    pub async fn run_worker(
        &self,
        mut request: AgentRequest,
        budget: usize,
        worker_timeout: Duration,
    ) -> Result<AgentRunResult> {
        if self.guard.is_some() && request.guard_pipeline.is_none() {
            request.guard_pipeline = self.guard.clone();
        }

        let sink = CollectingSink::new();
        let run = self.service.run(request, &sink);
        let result = timeout(worker_timeout, run)
            .await
            .map_err(|_| anyhow!("worker timed out after {:?}", worker_timeout))?
            .context("worker agent run failed")?;

        if budget > 0 {
            if let Some(usage) = result.usage.as_ref() {
                if usage.total_tokens as usize > budget {
                    tracing::warn!(
                        total_tokens = usage.total_tokens,
                        budget,
                        "research worker exceeded token budget"
                    );
                }
            }
        }

        Ok(result)
    }

    pub fn default_timeout() -> Duration {
        DEFAULT_WORKER_TIMEOUT
    }
}

/// Fan-out research queries across RAG and Search workers (spec §6.3, §7).
pub async fn research(
    invoker: &SubagentInvoker,
    parent: &AgentRequest,
    topic: &str,
    budget: &heavytail::state::WriterBudget,
) -> ResearchOutcome {
    let timeout = SubagentInvoker::default_timeout();
    let per_worker_budget = budget.research_tokens_per_worker;
    let service = Arc::clone(&invoker.service);
    let guard = parent
        .guard_pipeline
        .clone()
        .or_else(|| invoker.guard.clone());
    let trace_id = parent.session_id.clone();

    let mut join_set = tokio::task::JoinSet::new();
    let mut planned_workers = 0usize;
    let mut failed_workers = 0usize;

    if !parent.doc_scope.is_empty() {
        planned_workers += 1;
        let service = Arc::clone(&service);
        let parent = parent.clone();
        let queries = rag_research_queries(topic);
        join_set.spawn(async move {
            let invoker = SubagentInvoker::new(service, None);
            let kind = AgentKind::Rag;
            let mut last_err = None;
            let mut merged = AgentRunResult::default();
            for query in queries {
                let request = SubagentInvoker::worker_request(&parent, kind, &query);
                match invoker.run_worker(request, per_worker_budget, timeout).await {
                    Ok(result) => merge_worker_result(&mut merged, result),
                    Err(err) => {
                        tracing::warn!(worker = "rag", error = %err, "research worker query failed");
                        last_err = Some(err);
                    }
                }
            }
            if merged.answer.is_empty() && merged.citations.is_empty() {
                Err(last_err.unwrap_or_else(|| anyhow!("rag worker returned no results")))
            } else {
                Ok((AgentKind::Rag, merged))
            }
        });
    }

    planned_workers += 1;
    let service = Arc::clone(&service);
    let parent = parent.clone();
    let topic = topic.to_string();
    join_set.spawn(async move {
        let invoker = SubagentInvoker::new(service, None);
        let kind = AgentKind::Search;
        let mut last_err = None;
        let mut merged = AgentRunResult::default();
        for query in search_research_queries(&topic) {
            let request = SubagentInvoker::worker_request(&parent, kind, &query);
            match invoker.run_worker(request, per_worker_budget, timeout).await {
                Ok(result) => merge_worker_result(&mut merged, result),
                Err(err) => {
                    tracing::warn!(worker = "search", error = %err, "research worker query failed");
                    last_err = Some(err);
                }
            }
        }
        if merged.answer.is_empty() && merged.citations.is_empty() {
            Err(last_err.unwrap_or_else(|| anyhow!("search worker returned no results")))
        } else {
            Ok((AgentKind::Search, merged))
        }
    });

    let mut cards = Vec::new();
    let mut citations = Vec::new();
    let mut degrade_trace = Vec::new();

    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(Ok((kind, result))) => {
                let extracted = super::cards::extract_material_cards(
                    &result,
                    kind,
                    guard.as_deref(),
                    trace_id.as_deref(),
                );
                citations.extend(result.citations.clone());
                cards.extend(extracted.cards);
                degrade_trace.extend(extracted.degrade_trace);
            }
            Ok(Err(err)) => {
                failed_workers += 1;
                tracing::warn!(error = %err, "research worker failed");
            }
            Err(err) => {
                failed_workers += 1;
                tracing::warn!(error = %err, "research worker task join failed");
            }
        }
    }

    cards = super::cards::dedupe_cards(cards);
    let reservoir = super::cards::build_reservoir(&cards);
    let research_degraded = failed_workers > 0 || (planned_workers > 0 && cards.is_empty());

    ResearchOutcome {
        cards,
        citations,
        reservoir,
        research_degraded,
        degrade_trace,
    }
}

pub struct ResearchOutcome {
    pub cards: Vec<heavytail::skeleton::MaterialCard>,
    pub citations: Vec<contracts::chat::Citation>,
    pub reservoir: Vec<String>,
    pub research_degraded: bool,
    pub degrade_trace: Vec<contracts::chat::DegradeTraceItem>,
}

fn merge_worker_result(into: &mut AgentRunResult, mut from: AgentRunResult) {
    if into.answer.is_empty() {
        into.answer = from.answer;
    } else if !from.answer.is_empty() {
        into.answer.push_str("\n\n");
        into.answer.push_str(&from.answer);
    }
    into.citations.append(&mut from.citations);
    into.sources.append(&mut from.sources);
    into.degrade_trace.append(&mut from.degrade_trace);
    into.tool_results.append(&mut from.tool_results);
    if into.usage.is_none() {
        into.usage = from.usage.take();
    } else if let (Some(acc), Some(add)) = (&mut into.usage, from.usage) {
        acc.prompt_tokens = acc.prompt_tokens.saturating_add(add.prompt_tokens);
        acc.completion_tokens = acc
            .completion_tokens
            .saturating_add(add.completion_tokens);
        acc.total_tokens = acc.total_tokens.saturating_add(add.total_tokens);
        acc.request_count = acc.request_count.saturating_add(add.request_count);
    }
}

fn rag_research_queries(topic: &str) -> Vec<String> {
    vec![
        format!("关于「{topic}」的核心事实、数据与背景资料"),
        format!("「{topic}」相关的重要概念、术语与定义"),
        format!("「{topic}」在实践中的应用案例与具体细节"),
    ]
}

fn search_research_queries(topic: &str) -> Vec<String> {
    vec![
        format!("「{topic}」最新动态与权威报道"),
        format!("「{topic}」关键数据、统计与专家观点"),
        format!("「{topic}」争议点、对比与不同视角"),
    ]
}
