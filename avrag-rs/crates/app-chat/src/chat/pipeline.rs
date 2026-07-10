use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;

use crate::context::ChatContext;
use app_documents::{AuditAction, AuditRecord};
use common::AppError;
use contracts::chat::{ChatRequest, ChatResponse};

#[derive(Clone)]
pub(crate) struct StreamConfig {
    pub sender: UnboundedSender<contracts::chat::ChatEvent>,
    pub request_id: String,
    pub token: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatPreflight {
    pub trace_id: String,
    pub user_uuid: Uuid,
    pub notebook_uuid: Option<Uuid>,
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
    /// When true, `build_response` skips its own token emission to avoid duplication.
    #[serde(default)]
    pub tokens_emitted: bool,
    /// Whether Citations events were already emitted during mode-step execution.
    #[serde(default)]
    pub citations_emitted: bool,
}

/// Which product lane owns this pipeline run (ADR-0007 Phase B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineLane {
    /// Chat / RAG / Search via UnifiedAgent + ToolCatalog.
    Agent,
    /// Write refine ring — never via ToolCatalog / AgentApp.
    Write,
}

pub fn is_write_agent_type(agent_type: &str) -> bool {
    agent_type.eq_ignore_ascii_case("write")
}

pub(crate) async fn execute_chat_pipeline(
    state: ChatContext,
    request: ChatRequest,
) -> Result<ChatResponse, AppError> {
    info!(
        orchestrator = "pipeline",
        lane = "agent",
        "executing chat with linear pipeline"
    );
    run_pipeline(state, request, None, PipelineLane::Agent).await
}

pub(crate) async fn execute_chat_pipeline_stream(
    state: ChatContext,
    request: ChatRequest,
    request_id: String,
    sender: UnboundedSender<contracts::chat::ChatEvent>,
    token: CancellationToken,
) -> Result<(), AppError> {
    let stream_config = StreamConfig {
        sender,
        request_id,
        token,
    };
    info!(
        orchestrator = "pipeline",
        lane = "agent",
        "executing streaming chat with linear pipeline"
    );
    run_pipeline(state, request, Some(stream_config), PipelineLane::Agent)
        .await
        .map(|_| ())
}

/// Product Write entry — preflight/session shared with chat; mode dispatch is write-only.
pub(crate) async fn execute_write_pipeline(
    state: ChatContext,
    request: ChatRequest,
) -> Result<ChatResponse, AppError> {
    info!(
        orchestrator = "pipeline",
        lane = "write",
        "executing write with linear pipeline"
    );
    run_pipeline(state, request, None, PipelineLane::Write).await
}

pub(crate) async fn execute_write_pipeline_stream(
    state: ChatContext,
    request: ChatRequest,
    request_id: String,
    sender: UnboundedSender<contracts::chat::ChatEvent>,
    token: CancellationToken,
) -> Result<(), AppError> {
    let stream_config = StreamConfig {
        sender,
        request_id,
        token,
    };
    info!(
        orchestrator = "pipeline",
        lane = "write",
        "executing streaming write with linear pipeline"
    );
    run_pipeline(state, request, Some(stream_config), PipelineLane::Write)
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
                "write mode must enter via execute_write_pipeline, not agent chat pipeline",
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
    let session = state.resolve_chat_session(&request).await?;

    if let Some(ref config) = stream_config {
        let _ = config.sender.send(contracts::chat::ChatEvent::Start {
            request_id: config.request_id.clone(),
            session_id: session.id.clone(),
        });
        if let Some(guide) =
            crate::external_agent_guide::load_invoke_operation_guide(&request.agent_type)
        {
            let _ = config
                .sender
                .send(contracts::chat::ChatEvent::OperationGuide {
                    request_id: config.request_id.clone(),
                    guide,
                });
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
            crate::writer::run_write_mode(&state, &request, &session, stream_config.as_ref()).await?
        }
    };

    let audit_action = match execution.mode.as_str() {
        "search" => AuditAction::SearchRequest,
        "rag" => AuditAction::RagRequest,
        "write" => AuditAction::ChatRequest,
        _ => AuditAction::ChatRequest,
    };
    let audit_record = AuditRecord {
        audit_id: Uuid::new_v4().to_string(),
        org_id: state.auth.org_id().into_uuid().to_string(),
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

    // Emit terminal SSE (especially `done`) before persistence/usage so clients
    // are not blocked on slow post-processing (fixes missing done on long answers).
    crate::chat::pipeline_steps::emit_terminal_stream_events(stream_config.as_ref(), &execution);

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

    if request.source_type.as_deref() != Some("share") {
        state
            .emit_notifications_for_execution(&session, &execution)
            .await?;
    }

    Ok(crate::external_agent_guide::attach_operation_guide(
        execution.response,
    ))
}
