#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use avrag_rag_core::RagRuntime;
use graph_flow::{Context, GraphBuilder, GraphError, NextAction, Task, TaskResult};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::AppState;
use common::{AppError, ChatRequest, ChatResponse, ChatSession};

const APP_ERROR_PREFIX: &str = "APP_ERROR_JSON:";

const KEY_REQUEST: &str = "chat.request";
const KEY_PREFLIGHT: &str = "chat.preflight";
const KEY_SESSION: &str = "chat.session";
const KEY_EXECUTION: &str = "chat.execution";
const KEY_RESPONSE: &str = "chat.response";
const KEY_RAG_SESSION_CONTEXT: &str = "chat.rag_session_context";
const KEY_RAG_PLAN: &str = "chat.rag_plan";
const KEY_RAG_EXECUTE_RESPONSE: &str = "chat.rag_execute_response";
const KEY_DOCSCOPE_METADATA: &str = "chat.docscope_metadata";
const KEY_TEXT_DENSE_LISTS: &str = "chat.text_dense_lists";
const KEY_BM25_LISTS: &str = "chat.bm25_lists";
const KEY_MULTIMODAL_POOL: &str = "chat.multimodal_pool";
const KEY_TEXT_POOL: &str = "chat.text_pool";
const KEY_RERANKED_CHUNKS: &str = "chat.reranked_chunks";
const KEY_SUMMARY_CHUNKS: &str = "chat.summary_chunks";
const KEY_ANSWER_CONTEXT: &str = "chat.answer_context";
const KEY_RETRIEVED_CHUNKS: &str = "chat.retrieved_chunks";
const KEY_ITEM_TRACE: &str = "chat.item_trace";
const KEY_DEGRADE_TRACE: &str = "chat.degrade_trace";

const TASK_PREFLIGHT: &str = "chat_preflight";
const TASK_SESSION: &str = "chat_session";
const TASK_MODE_SELECT: &str = "chat_mode_select";
const TASK_MEMORY_COMPAT: &str = "chat_memory_compat";
const TASK_GENERAL: &str = "chat_run_general";
const TASK_SEARCH: &str = "chat_run_search";
const TASK_RAG_LOAD_SESSION_CONTEXT: &str = "rag_load_session_context";
const TASK_RAG_PREPARE_PLANNER_INPUT: &str = "rag_prepare_planner_input";
const TASK_RAG_CALL_PLANNER: &str = "rag_call_planner";
const TASK_RAG_NORMALIZE_PLAN: &str = "rag_normalize_plan";
const TASK_RAG_EXECUTE_PLAN: &str = "rag_execute_plan";
const TASK_RAG_RETRIEVE_TEXT_DENSE: &str = "rag_retrieve_text_dense";
const TASK_RAG_RETRIEVE_BM25: &str = "rag_retrieve_bm25";
const TASK_RAG_RETRIEVE_MULTIMODAL_DENSE: &str = "rag_retrieve_multimodal_dense";
const TASK_RAG_MERGE_TEXT_RRF: &str = "rag_merge_text_rrf";
const TASK_RAG_MULTIMODAL_RERANK: &str = "rag_multimodal_rerank";
const TASK_RAG_CUT_FINAL_CANDIDATES: &str = "rag_cut_final_candidates";
const TASK_RAG_APPLY_SUMMARY_POLICY: &str = "rag_apply_summary_policy";
const TASK_RAG_BUILD_ANSWER_CONTEXT: &str = "rag_build_answer_context";
const TASK_RAG_ANSWER_SYNTHESIZE: &str = "rag_answer_synthesize";
const TASK_RAG_VALIDATE_CITATIONS: &str = "rag_validate_citations";
const TASK_OUTPUT_GUARD: &str = "chat_output_guard";
const TASK_PERSIST: &str = "chat_persist";
const TASK_USAGE: &str = "chat_record_usage";
const TASK_NOTIFY: &str = "chat_emit_notifications";
const TASK_BUILD_RESPONSE: &str = "chat_build_response";

include!("graphflow_context.rs");

pub(crate) async fn execute_graphflow_chat(
    state: AppState,
    request: ChatRequest,
) -> Result<ChatResponse, AppError> {
    let graph = build_chat_graph(state);
    let context = ChatFlowContext::from(Context::new());
    context.set_request(&request).await;

    let result = graph
        .execute(TASK_PREFLIGHT, context.0.clone())
        .await
        .map_err(map_graphflow_error)?;

    if let Some(response) = context.response().await {
        return Ok(response);
    }

    let raw_value = context.raw_response().await;
    Err(AppError::internal(format!(
        "graphflow chat completed without a final response: task_response={:?}, raw_context={:?}",
        result.response, raw_value
    )))
}

fn build_chat_graph(state: AppState) -> graph_flow::Graph {
    GraphBuilder::new("chat_orchestration_graph")
        .add_task(Arc::new(PreflightTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(SessionTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(ModeSelectTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(MemoryCompatTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(GeneralModeTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(SearchModeTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagPreparePlannerInputTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagCallPlannerTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagNormalizePlanTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagExecutePlanTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagAnswerSynthesizeTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RagValidateCitationsTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(OutputGuardTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(PersistTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(RecordUsageTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(EmitNotificationsTask {
            state: state.clone(),
        }))
        .add_task(Arc::new(BuildResponseTask))
        .add_edge(TASK_PREFLIGHT, TASK_SESSION)
        .add_edge(TASK_SESSION, TASK_MODE_SELECT)
        .add_edge(TASK_MODE_SELECT, TASK_MEMORY_COMPAT)
        .add_edge(TASK_MODE_SELECT, TASK_GENERAL)
        .add_edge(TASK_MODE_SELECT, TASK_SEARCH)
        .add_edge(TASK_MODE_SELECT, TASK_RAG_PREPARE_PLANNER_INPUT)
        .add_edge(TASK_MEMORY_COMPAT, TASK_OUTPUT_GUARD)
        .add_edge(TASK_RAG_PREPARE_PLANNER_INPUT, TASK_RAG_CALL_PLANNER)
        .add_edge(TASK_RAG_CALL_PLANNER, TASK_RAG_NORMALIZE_PLAN)
        .add_edge(TASK_RAG_NORMALIZE_PLAN, TASK_RAG_EXECUTE_PLAN)
        .add_edge(TASK_RAG_EXECUTE_PLAN, TASK_RAG_ANSWER_SYNTHESIZE)
        .add_edge(TASK_RAG_ANSWER_SYNTHESIZE, TASK_RAG_VALIDATE_CITATIONS)
        .add_edge(TASK_GENERAL, TASK_OUTPUT_GUARD)
        .add_edge(TASK_SEARCH, TASK_OUTPUT_GUARD)
        .add_edge(TASK_RAG_VALIDATE_CITATIONS, TASK_OUTPUT_GUARD)
        .add_edge(TASK_OUTPUT_GUARD, TASK_PERSIST)
        .add_edge(TASK_PERSIST, TASK_USAGE)
        .add_edge(TASK_USAGE, TASK_NOTIFY)
        .add_edge(TASK_NOTIFY, TASK_BUILD_RESPONSE)
        .build()
}

fn graph_app_error(error: AppError) -> GraphError {
    let payload = serde_json::to_string(&FlowAppErrorData::from(error))
        .unwrap_or_else(|serialization_error| serialization_error.to_string());
    GraphError::TaskExecutionFailed(format!("{APP_ERROR_PREFIX}{payload}"))
}

fn map_graphflow_error(error: GraphError) -> AppError {
    match error {
        GraphError::TaskExecutionFailed(payload) => {
            if let Some(raw) = payload.strip_prefix(APP_ERROR_PREFIX) {
                return serde_json::from_str::<FlowAppErrorData>(raw)
                    .map(FlowAppErrorData::into_app_error)
                    .unwrap_or_else(|_| AppError::internal(payload));
            }
            AppError::internal(payload)
        }
        other => AppError::internal(format!("graphflow chat execution failed: {other}")),
    }
}

fn require_rag_runtime(state: &AppState) -> graph_flow::Result<Arc<RagRuntime>> {
    state.rag_runtime.as_ref().cloned().ok_or_else(|| {
        graph_app_error(AppError::validation(
            "rag_runtime_not_configured",
            "RAG mode requires rag_runtime to be configured.",
        ))
    })
}

async fn append_degrade_trace(flow: &ChatFlowContext, mut trace: Vec<common::DegradeTraceItem>) {
    if trace.is_empty() {
        return;
    }

    let mut existing = flow.degrade_trace().await.unwrap_or_default();
    existing.append(&mut trace);
    flow.set_degrade_trace(&existing).await;
}

include!("graphflow_tasks.rs");
