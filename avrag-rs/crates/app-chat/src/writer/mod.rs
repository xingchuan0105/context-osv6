//! HeavyTail write-mode orchestrator glue (spec §6).

mod adapters;
mod cards;
mod invoker;
mod material_pack;
mod refine_loop;

pub use material_pack::MaterialPack;
pub use refine_loop::{
    run_write_refine, RefineContext, RefineLoopBudget, WriteRefineLoopRunner, WRITE_REFINE_HARD_REACT_CAP,
};
pub use write_core::{WRITE_AGENT_TYPE, WRITE_MODE};

pub use invoker::{research, ResearchOutcome, SubagentInvoker};

use std::path::PathBuf;

use common::AppError;
use heavytail::draft::{self, PRIMING};
use heavytail::llm::WriterLlm;
use heavytail::metrics::analyze_sentences;
use heavytail::skeleton;
use heavytail::state::{WriterBudget, WriterPhase, WriterState};
use heavytail::validator;
use heavytail::StyleParams;
use tracing::warn;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::progressive::PromptRegistry;
use crate::agents::runtime::{AgentRequest, AgentRunResult};
use crate::context::ChatContext;

const DEFAULT_TARGET_CHARS: usize = 2_000;
const HEAVYTAIL_PRIMING_SKILL_ID: &str = "heavytail-priming";

/// Build `WriterLlm` from bootstrap agent client + exit metering when available.
/// Falls back to `AGENT_LLM_*` env for offline / experiment paths.
fn build_writer_llm(ctx: &ChatContext) -> Result<WriterLlm, AppError> {
    use avrag_llm::TenantContext;
    use uuid::Uuid;

    let mut client = match ctx.llm_ctx.agent_client().cloned() {
        Some(client) => client,
        None => {
            return WriterLlm::from_env().map_err(|e| {
                AppError::internal(format!("writer LLM configuration error: {e}"))
            });
        }
    };

    if let Some(observer) = ctx.billing.usage_observer() {
        let tenant = TenantContext {
            org_id: ctx.auth.org_id().into_uuid(),
            user_id: ctx
                .auth
                .actor_id()
                .map(|a| a.into_uuid())
                .unwrap_or_else(Uuid::nil),
        };
        client = client.with_observer(observer.clone(), tenant);
    }

    Ok(WriterLlm::from_client(client))
}

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
        write_core::require_non_empty_write_topic(&topic)?;

        let style = StyleParams::default();
        let budget = WriterBudget::default();
        let mut state = WriterState::default();
        let checkpoint_dir = writer_checkpoint_dir(request.session_id.as_deref());
        let priming = load_write_priming();

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

        let llm = build_writer_llm(self.ctx)?;
        let skeleton_llm = llm.with_phase("skeleton");
        let target_chars = DEFAULT_TARGET_CHARS;
        let skeleton = skeleton::plan_skeleton(
            &skeleton_llm,
            &topic,
            target_chars,
            &state.cards,
            &mut state.tokens_used,
        )
        .await
        .map_err(|e| AppError::internal(format!("skeleton planning failed: {e}")))?;
        state.skeleton = Some(skeleton.clone());
        state.phase = WriterPhase::Drafting { section: 0 };
        checkpoint_state(&state, &checkpoint_dir)?;

        let draft_llm = llm.with_phase("draft");
        let section_sink = sink.clone_boxed();
        draft::draft_sections(
            &draft_llm,
            &skeleton,
            &style,
            &state.cards,
            &mut state.workspace,
            &draft::DraftOptions {
                mpc: true,
                primed: true,
                priming: Some(priming.as_str()),
                on_section: Some(&|section, total| {
                    spawn_section_progress(section_sink.as_ref(), section, total);
                }),
                ..Default::default()
            },
            &mut state.tokens_used,
        )
        .await
        .map_err(|e| AppError::internal(format!("section drafting failed: {e}")))?;
        state.phase = WriterPhase::Refining { round: 0 };
        checkpoint_state(&state, &checkpoint_dir)?;

        // Default-on WriteRefine agent loop (replaces fixed-round heavytail::refine).
        emit_activity(sink, "refine", "Starting WriteRefine agent loop").await;
        let refine_llm = llm.with_phase("refine");
        let reservoir = research_outcome.reservoir.clone();
        let mut workspace = std::mem::take(&mut state.workspace);
        let material_pack =
            material_pack::MaterialPack::from_research(
                &research_outcome.materials(),
                &workspace.render_plain(),
            );
        let diagnosis =
            heavytail::diagnosis::diagnose_pre_refine(&workspace, &style, &reservoir);
        let mut refine_ctx = RefineContext::new(
            std::mem::take(&mut workspace),
            diagnosis,
            material_pack,
            None,
        );
        let refine_budget =
            RefineLoopBudget::from_writer_budget(&budget, WRITE_REFINE_HARD_REACT_CAP);
        run_write_refine(
            &refine_llm,
            &invoker,
            &request,
            style.clone(),
            refine_budget,
            &mut refine_ctx,
            &reservoir,
            &mut state,
            sink,
            &checkpoint_dir,
        )
            .await
            .map_err(|e| AppError::internal(format!("write refine loop failed: {e}")))?;
        state.workspace = refine_ctx.workspace;
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
        state.phase = WriterPhase::Done;
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
                "priming_skill": HEAVYTAIL_PRIMING_SKILL_ID,
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

fn load_write_priming() -> String {
    let registry = CapabilityRegistry::standard_cached();
    let available = registry
        .answer_writing_styles("write")
        .into_iter()
        .any(|skill| skill.id == HEAVYTAIL_PRIMING_SKILL_ID);
    if available {
        if let Some(skill) = PromptRegistry::standard_cached().skill(HEAVYTAIL_PRIMING_SKILL_ID) {
            let body = skill.system_prompt().trim();
            if !body.is_empty() {
                return body.to_string();
            }
        }
    }
    PRIMING.to_string()
}

fn spawn_section_progress(sink: &dyn AgentEventSink, section: usize, total: usize) {
    let sink = sink.clone_boxed();
    let message = format!("Drafting section {section}/{total}");
    tokio::spawn(async move {
        emit_activity(sink.as_ref(), "draft", &message).await;
    });
}

fn spawn_round_progress(sink: &dyn AgentEventSink, round: usize, max_rounds: usize) {
    let sink = sink.clone_boxed();
    let message = format!("Refining round {round}/{max_rounds}");
    tokio::spawn(async move {
        emit_activity(sink.as_ref(), "refine", &message).await;
    });
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
            WRITE_AGENT_TYPE.to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = orchestrator.run(agent_request, &sink).await?;
        crate::emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        let mut execution = crate::chat::build_chat_execution_from_result(
            &agent_result,
            crate::chat::BuildChatExecutionParams {
                mode: WRITE_MODE,
                agent_type: WRITE_AGENT_TYPE,
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
            mode: WRITE_MODE,
            agent_type: WRITE_AGENT_TYPE,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_write_priming_uses_skill_or_fallback() {
        let priming = load_write_priming();
        assert!(
            priming.contains("长短交错") || priming == PRIMING,
            "expected heavytail priming content, got: {priming:?}"
        );
    }
}
