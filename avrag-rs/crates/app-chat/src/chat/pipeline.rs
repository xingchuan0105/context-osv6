use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;

use crate::context::ChatContext;
use app_documents::{AuditAction, AuditRecord};
use common::AppError;
use contracts::chat::{ChatRequest, ChatResponse};

#[derive(Clone)]
pub(crate) struct StreamConfig {
    pub sender: Sender<contracts::chat::ChatEvent>,
    pub request_id: String,
    pub token: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatPreflight {
    pub trace_id: String,
    pub user_uuid: Uuid,
    pub notebook_uuid: Option<Uuid>,
    /// Wallet hold placed for estimated platform spend (released after turn).
    #[serde(default)]
    pub usage_hold_id: Option<Uuid>,
    #[serde(default)]
    pub usage_hold_fen: Option<i64>,
    /// Token estimate used for hold placement after all gates pass.
    #[serde(default)]
    pub estimated_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatExecution {
    pub mode: String,
    pub input_usage_text: String,
    pub apply_output_guard: bool,
    pub response: ChatResponse,
    pub llm_usage: Option<avrag_llm::LlmUsage>,
    #[serde(default)]
    pub debug_metadata: Option<serde_json::Value>,
    /// Whether Token events were already emitted during mode-step execution.
    #[serde(default)]
    pub tokens_emitted: bool,
    /// Whether Citations events were already emitted during mode-step execution.
    #[serde(default)]
    pub citations_emitted: bool,
    /// Assistant-row `turn_metadata` (e.g. `{ "progress": { … } }`) for refresh restore.
    #[serde(default)]
    pub assistant_turn_metadata: Option<serde_json::Value>,
}

/// Which product lane owns this pipeline run (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineLane {
    /// Chat / RAG / Search via UnifiedAgent + ToolCatalog.
    Agent,
    /// Write refine ring — never via ToolCatalog.
    Write,
}

pub fn is_write_agent_type(agent_type: &str) -> bool {
    agent_type.eq_ignore_ascii_case("write")
}

/// Internal WriteRefine control-ring id — not a user-selectable product mode.
///
/// Callers must use `agent_type=write`; refine tools run only inside the write
/// pipeline (`write_refine::tool_specs_for_pool`), never as a top-level mode.
pub fn is_reserved_internal_agent_type(agent_type: &str) -> bool {
    agent_type.eq_ignore_ascii_case("write_refine")
}

/// Non-streaming pipeline for either product lane.
pub(crate) async fn execute_pipeline(
    state: ChatContext,
    request: ChatRequest,
    lane: PipelineLane,
) -> Result<ChatResponse, AppError> {
    info!(
        orchestrator = "pipeline",
        lane = ?lane,
        "executing linear pipeline"
    );
    run_pipeline(state, request, None, lane).await
}

/// Streaming pipeline for either product lane.
pub(crate) async fn execute_pipeline_stream(
    state: ChatContext,
    request: ChatRequest,
    request_id: String,
    sender: Sender<contracts::chat::ChatEvent>,
    token: CancellationToken,
    lane: PipelineLane,
) -> Result<(), AppError> {
    let stream_config = StreamConfig {
        sender,
        request_id,
        token,
    };
    info!(
        orchestrator = "pipeline",
        lane = ?lane,
        "executing streaming linear pipeline"
    );
    run_pipeline(state, request, Some(stream_config), lane)
        .await
        .map(|_| ())
}

async fn run_pipeline(
    state: ChatContext,
    request: ChatRequest,
    stream_config: Option<StreamConfig>,
    lane: PipelineLane,
) -> Result<ChatResponse, AppError> {
    match lane {
        PipelineLane::Agent if is_write_agent_type(&request.agent_type) => {
            return Err(AppError::validation(
                "use_write_entry",
                "write mode must enter via write pipeline lane, not agent chat pipeline",
            ));
        }
        PipelineLane::Write if !is_write_agent_type(&request.agent_type) => {
            return Err(AppError::validation(
                "write_mode_required",
                "write pipeline only accepts agent_type=write",
            ));
        }
        _ => {}
    }

    let preflight = state.execute_chat_preflight(&request).await?;
    // Hold is placed inside after cache miss; always released on return.
    let mut active_hold: Option<(Uuid, i64)> = None;
    let outcome =
        run_pipeline_inner(state.clone(), request, stream_config, lane, preflight, &mut active_hold)
            .await;
    if let Some((hold_id, hold_fen)) = active_hold {
        state
            .billing
            .release_usage_hold(&state.auth, hold_id, hold_fen)
            .await;
    }
    outcome
}

async fn run_pipeline_inner(
    state: ChatContext,
    request: ChatRequest,
    stream_config: Option<StreamConfig>,
    lane: PipelineLane,
    preflight: ChatPreflight,
    active_hold: &mut Option<(Uuid, i64)>,
) -> Result<ChatResponse, AppError> {
    let session = state.resolve_chat_session(&request).await?;

    // ADR-0010 §9: exact first (no embed), then semantic with embed.
    if request.source_type.as_deref() == Some("share") {
        if let Some(token) = request.source_token.as_deref() {
            // Exact match: no embedding call (no platform embed spend / no hold).
            if let Some(cached) =
                crate::share_cache::lookup(token, &request.query, None).await
            {
                return emit_share_cache_hit(
                    &session,
                    &request,
                    stream_config.as_ref(),
                    cached,
                )
                .await;
            }
            // Semantic: embed only when exact miss and rag runtime available.
            let embed = async {
                let Some(rag) = state.retrieval_runtime() else {
                    return None;
                };
                match rag
                    .embedding_client()
                    .embed(&[request.query.as_str()])
                    .await
                {
                    Ok(mut v) => v.pop(),
                    Err(e) => {
                        tracing::debug!(error = %e, "share cache embed failed; exact-only");
                        None
                    }
                }
            }
            .await;
            if let Some(cached) =
                crate::share_cache::lookup(token, &request.query, embed.as_deref()).await
            {
                return emit_share_cache_hit(
                    &session,
                    &request,
                    stream_config.as_ref(),
                    cached,
                )
                .await;
            }
        }
    }

    // All gates passed and cache miss → atomic hold for estimated platform spend.
    if let Some((id, fen)) = state
        .billing
        .place_usage_hold_for_estimate(
            &state.auth,
            preflight.estimated_input_tokens,
            1024,
        )
        .await?
    {
        *active_hold = Some((id, fen));
    }

    if let Some(ref config) = stream_config {
        let _ = config.sender.send(contracts::chat::ChatEvent::Start {
            request_id: config.request_id.clone(),
            session_id: session.id.clone(),
        }).await;
        if let Some(guide) =
            crate::external_agent_guide::load_invoke_operation_guide(&request.agent_type)
        {
            let _ = config
                .sender
                .send(contracts::chat::ChatEvent::OperationGuide {
                    request_id: config.request_id.clone(),
                    guide,
                })
                .await;
        }
    }

    let mut execution = match lane {
        PipelineLane::Agent => {
            crate::chat::pipeline_steps::dispatch_agent_mode(
                &state,
                &request,
                &session,
                stream_config.as_ref(),
            )
            .await?
        }
        PipelineLane::Write => {
            crate::writer::run_write_mode(&state, &request, &session, stream_config.as_ref())
                .await?
        }
    };

    let audit_action = match execution.mode.as_str() {
        "search" => AuditAction::SearchRequest,
        "rag" => AuditAction::RagRequest,
        _ => AuditAction::ChatRequest,
    };
    let audit_record = AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        owner_user_id: state.auth.user_id().into_uuid().to_string(),
        actor_id: preflight.user_uuid.to_string().into(),
        action: audit_action,
        resource_type: "chat".to_string(),
        resource_id: session.id.clone(),
        payload: serde_json::json!({
            "mode": execution.mode,
            "agent_type": request.agent_type,
            "trace_id": preflight.trace_id,
            "workspace_id": session.workspace_id,
            "lane": match lane {
                PipelineLane::Agent => "agent",
                PipelineLane::Write => "write",
            },
        }),
        created_at: common::now_rfc3339(),
    };
    if let Some(chat_persistence) = state.chat_persistence() {
        let _ = chat_persistence.append_audit_record(&audit_record).await;
    }

    if execution.apply_output_guard {
        state
            .apply_output_guard_to_execution(
                &session,
                &mut execution,
                &preflight.trace_id,
                preflight.user_uuid,
                state.chat_persistence().as_deref(),
            )
            .await?;
    }

    crate::chat::pipeline_steps::emit_terminal_stream_events(stream_config.as_ref(), &execution).await;

    if request.source_type.as_deref() == Some("share") {
        if let Some(token) = request.source_token.as_deref() {
            let embed = async {
                let Some(rag) = state.retrieval_runtime() else {
                    return None;
                };
                match rag
                    .embedding_client()
                    .embed(&[request.query.as_str()])
                    .await
                {
                    Ok(mut v) => v.pop(),
                    Err(_) => None,
                }
            }
            .await;
            crate::share_cache::store(
                token,
                &request.query,
                &execution.response.answer,
                embed,
            )
            .await;
        }
    }

    if request.source_type.as_deref() != Some("share")
        && let Some(chat_persistence) = state.chat_persistence()
    {
        state
            .persist_chat_execution(
                &request,
                &session,
                &mut execution,
                chat_persistence.as_ref(),
            )
            .await?;
    }

    state.record_usage_for_execution(&execution).await?;

    Ok(crate::external_agent_guide::attach_operation_guide(
        execution.response,
    ))
}

async fn emit_share_cache_hit(
    session: &contracts::workspaces::ChatSession,
    request: &ChatRequest,
    stream_config: Option<&StreamConfig>,
    cached: String,
) -> Result<ChatResponse, AppError> {
    let response = ChatResponse {
        answer: cached.clone(),
        answer_blocks: vec![],
        session_id: session.id.clone(),
        agent_type: request.agent_type.clone(),
        sources: vec![],
        citations: vec![],
        trace: contracts::chat::TraceInfo {
            mode: "share_cache".to_string(),
        },
        degrade_trace: vec![],
        planner_output: None,
        mode_debug: None,
        message_id: None,
        guard_report: None,
        tool_results: vec![],
        usage: None,
        agent_operation_guide: None,
    };
    if let Some(config) = stream_config {
        let mid = crate::stream_event_message_id(None);
        let _ = config.sender.send(contracts::chat::ChatEvent::Start {
            request_id: config.request_id.clone(),
            session_id: session.id.clone(),
        }).await;
        for chunk in crate::chunk_text_for_stream(&cached) {
            let _ = config.sender.send(contracts::chat::ChatEvent::Token {
                request_id: config.request_id.clone(),
                message_id: mid,
                content: chunk,
            }).await;
        }
        let _ = config.sender.send(contracts::chat::ChatEvent::Done {
            request_id: config.request_id.clone(),
            session_id: session.id.clone(),
            message_id: mid,
            payload: crate::chat_done_payload(&response),
        }).await;
    }
    Ok(response)
}
