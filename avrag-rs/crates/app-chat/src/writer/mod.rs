//! HeavyTail write-mode orchestrator glue (spec §6).

mod cards;
mod invoker;

pub use invoker::{research, ResearchOutcome, SubagentInvoker};

use std::path::PathBuf;

use common::AppError;
use heavytail::draft::{self, PRIMING};
use heavytail::llm::WriterLlm;
use heavytail::metrics::analyze_sentences;
use heavytail::skeleton;
use heavytail::state::{WriterBudget, WriterPhase, WriterState};
use heavytail::validator;
use heavytail::workspace::DraftWorkspace;
use heavytail::StyleParams;
use tracing::warn;

use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{AgentRequest, AgentRunResult};
use crate::context::ChatContext;

const DEFAULT_TARGET_CHARS: usize = 2_000;

pub struct WriterOrchestrator<'a> {
    ctx: &'a ChatContext,
}

impl<'a> WriterOrchestrator<'a> {
    pub fn new(ctx: &'a ChatContext) -> Self {
        Self { ctx }
    }

    /// research → skeleton → draft → refine → validate → AgentRunResult
    pub async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let Some(service) = self.ctx.agent_service() else {
            return Err(AppError::internal("agent service is not configured"));
        };

        let topic = request.query.trim().to_string();
        if topic.is_empty() {
            return Err(AppError::validation(
                "empty_write_topic",
                "Write mode requires a non-empty topic",
            ));
        }

        let style = StyleParams::default();
        let budget = WriterBudget::default();
        let mut state = WriterState::default();
        let checkpoint_dir = writer_checkpoint_dir(request.session_id.as_deref());

        emit_activity(sink, "research", "Gathering research material").await;

        let invoker = SubagentInvoker::new(
            service,
            Some(self.ctx.orchestrator.guard_pipeline().clone()),
        );
        let research_outcome = research(&invoker, &request, &topic, &budget).await;
        state.cards = research_outcome.cards.clone();
        state.phase = WriterPhase::Skeleton;
        checkpoint_state(&state, &checkpoint_dir)?;

        emit_activity(sink, "skeleton", "Planning article outline").await;

        let llm = WriterLlm::from_env().map_err(|e| {
            AppError::internal(format!("writer LLM configuration error: {e}"))
        })?;
        let target_chars = DEFAULT_TARGET_CHARS;
        let skeleton = skeleton::plan_skeleton(&llm, &topic, target_chars, &state.cards)
            .await
            .map_err(|e| AppError::internal(format!("skeleton planning failed: {e}")))?;
        state.skeleton = Some(skeleton.clone());
        state.phase = WriterPhase::Drafting { section: 0 };
        checkpoint_state(&state, &checkpoint_dir)?;

        emit_activity(sink, "draft", "Drafting sections").await;

        let mut workspace = DraftWorkspace::default();
        draft::draft_sections(
            &llm,
            &skeleton,
            &style,
            &state.cards,
            &mut workspace,
            true,
            true,
            false,
        )
            .await
            .map_err(|e| AppError::internal(format!("section drafting failed: {e}")))?;
        state.workspace = workspace;
        state.phase = WriterPhase::Refining { round: 0 };
        checkpoint_state(&state, &checkpoint_dir)?;

        emit_activity(sink, "refine", "Refining draft").await;

        let reservoir = research_outcome.reservoir.clone();
        let mut workspace = std::mem::take(&mut state.workspace);
        heavytail::refine::refine(
            &llm,
            &mut workspace,
            &style,
            &reservoir,
            &budget,
            &mut state,
        )
        .await
        .map_err(|e| AppError::internal(format!("refinement failed: {e}")))?;
        state.workspace = workspace;
        checkpoint_state(&state, &checkpoint_dir)?;

        emit_activity(sink, "validate", "Validating fingerprint bands").await;

        state.phase = WriterPhase::Validating;
        let sentences: Vec<(String, usize)> = state
            .workspace
            .live()
            .map(|s| (s.text.clone(), s.para))
            .collect();
        let fingerprint = analyze_sentences(&sentences);
        let validation = validator::validate(&fingerprint, &style);
        state.phase = if validation.passed {
            WriterPhase::Done
        } else {
            WriterPhase::Done
        };
        checkpoint_state(&state, &checkpoint_dir)?;

        let answer = state.workspace.render_plain();
        let citations = cards::filter_citations_for_cards(
            &research_outcome.citations,
            &state.cards,
            &skeleton,
        );

        let mut degrade_trace = research_outcome.degrade_trace;
        if research_outcome.research_degraded {
            degrade_trace.push(contracts::chat::DegradeTraceItem {
                stage: "write:research".into(),
                reason: contracts::chat::DegradeReason::ToolDegraded,
                impact: "research_degraded".into(),
            });
        }
        if !validation.passed {
            degrade_trace.push(contracts::chat::DegradeTraceItem {
                stage: "write:validate".into(),
                reason: contracts::chat::DegradeReason::Other("validation_warning".into()),
                impact: "fingerprint bands not fully satisfied".into(),
            });
        }

        let debug_payload = serde_json::json!({
            "write_result": {
                "fingerprint": fingerprint,
                "validation": validation,
                "rounds_used": state.rounds.len(),
                "total_tokens": state.tokens_used,
                "research_degraded": research_outcome.research_degraded,
                "validation_warning": !validation.passed,
                "checkpoint_dir": checkpoint_dir.display().to_string(),
                "priming": PRIMING,
            }
        });

        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(answer.clone()),
                usage: None,
            })
            .await;

        Ok(AgentRunResult {
            answer,
            citations,
            degrade_trace,
            debug_payload: Some(debug_payload),
            routing_decision: Some("write".to_string()),
            ..Default::default()
        })
    }
}

async fn emit_activity(sink: &dyn AgentEventSink, stage: &str, message: &str) {
    let _ = sink
        .emit(AgentEvent::Activity {
            stage: stage.to_string(),
            message: message.to_string(),
        })
        .await;
}

fn writer_checkpoint_dir(session_id: Option<&str>) -> PathBuf {
    let mut dir = std::env::temp_dir().join("heavytail-writer");
    dir.push(session_id.unwrap_or("anonymous"));
    dir
}

fn checkpoint_state(state: &WriterState, dir: &PathBuf) -> Result<(), AppError> {
    if let Err(err) = state.checkpoint(dir) {
        warn!(error = %err, dir = %dir.display(), "writer checkpoint failed");
    }
    Ok(())
}

/// Run write mode from the chat pipeline (streaming and non-streaming).
pub(crate) async fn run_write_mode(
    state: &ChatContext,
    request: &contracts::chat::ChatRequest,
    session: &contracts::notebooks::ChatSession,
    stream_config: Option<&crate::chat::StreamConfig>,
) -> Result<crate::chat::ChatExecution, AppError> {
    let mut agent_request = state
        .build_agent_request(
            request,
            crate::agents::AgentKind::Write,
            Some(session.id.clone()),
        )
        .await;
    agent_request.guard_pipeline = Some(state.orchestrator.guard_pipeline().clone());

    if !request.doc_scope.is_empty()
        && let Ok(metadata) = state.load_docscope_metadata(&request.doc_scope).await
    {
        agent_request.docscope_metadata = Some(metadata);
    }

    if let Some(config) = stream_config {
        agent_request.stream = true;
        agent_request.cancellation_token = Some(config.token.clone());
    }

    let orchestrator = WriterOrchestrator::new(state);
    let emit_debug_trace = agent_request.debug;

    if let Some(config) = stream_config {
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            config.sender.clone(),
            config.request_id.clone(),
            session.id.clone(),
            crate::chat_streaming::STREAM_PLACEHOLDER_MESSAGE_ID,
            "write".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = orchestrator.run(agent_request, &sink).await?;
        crate::emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        let mut execution = crate::chat::build_chat_execution_from_result(
            &agent_result,
            crate::chat::BuildChatExecutionParams {
                mode: "write",
                agent_type: "write",
                session_id: &session.id,
                input_usage_text: request.query.trim(),
                apply_output_guard: true,
                mode_debug: None,
                debug_metadata: agent_result.debug_payload.clone(),
            },
        );
        execution.tokens_emitted = true;
        execution.citations_emitted = sink.has_citations_emitted();
        return Ok(execution);
    }

    let sink = crate::agents::events::CollectingSink::new();
    let agent_result = orchestrator.run(agent_request, &sink).await?;

    let mut execution = crate::chat::build_chat_execution_from_result(
        &agent_result,
        crate::chat::BuildChatExecutionParams {
            mode: "write",
            agent_type: "write",
            session_id: &session.id,
            input_usage_text: request.query.trim(),
            apply_output_guard: true,
            mode_debug: None,
            debug_metadata: agent_result.debug_payload.clone(),
        },
    );
    if emit_debug_trace {
        crate::chat::attach_debug_trace_from_sink(&mut execution, &sink);
    }
    Ok(execution)
}
