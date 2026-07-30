//! Orchestrator host: materialize → dispatch workers → **Answer phase**.
//!
//! O1: first wave runs **all** materialized channels with [`default_brief`]
//! (§7.1 structure + §7.2 invariant by construction). Multi-hop LLM re-dispatch is O2.
//!
//! V1 (evidence store): worker tool results are normalized into a shared
//! [`EvidenceStore`] (monotonic `E{n}` ids, doc identity joined from docscope
//! metadata); the Answer phase receives listings + worker digests and cites
//! `[[E:id]]`; the host rewrites markers to product citations after the run.
//!
//! Option D (2026-07-20): Answer phase assembles the Answer pack
//! (`product-answer-base` + material blocks; P1-2: no full `chat-base`) with
//! utility tools and prose-only contract — same Product Agent runtime, second
//! phase run (not a separate flat-chat agent product path).

use agent_loop::events::{AgentEventSink, CollectingSink};
use agent_loop::runtime::{AgentRequest, AgentRunResult};
use async_trait::async_trait;
use common::AppError;

use super::chat_exit::{direct_handoff, query_for_agent, synthesize_handoff};
use super::invariant::{assert_complete, default_brief};
use super::materialize::materialize_channels;
use super::store::{EvidenceKind, EvidenceStore};
use super::worker_session::{SessionError, WorkerSession};
use super::types::{
    Channel, ChannelNote, ChatHandoff, DispatchRecord, PackStatus, TaskBrief,
};
use super::workers::{
    attach_worker_thinking_events, finalize_answer_evidence, tool_failures,
    worker_observability_from_run, WorkerBriefObservability,
};
use crate::capabilities::CapabilitySet;

/// Abstraction so tests can mock channel + chat runs without LLM.
#[async_trait]
pub trait OrchestratorExecutor: Send + Sync {
    async fn run_channel(
        &self,
        channel: Channel,
        brief: &TaskBrief,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError>;

    /// Answer phase (sole user-facing answer). `sink` receives live answer tokens
    /// when `base.stream` is true — do not use a private CollectingSink here or
    /// the client freezes until the whole answer is ready.
    async fn run_chat(
        &self,
        handoff: &ChatHandoff,
        base: &AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError>;
}

/// Result of an orchestrated turn.
#[derive(Debug)]
pub struct OrchestratedTurn {
    pub answer_result: AgentRunResult,
    pub store: EvidenceStore,
    pub records: Vec<DispatchRecord>,
    pub handoff: ChatHandoff,
    pub agent_type_label: String,
    /// Per-channel sub-agent white-box (real tools + thinking). Not the store
    /// eval bridge on `answer_result.tool_results`.
    pub worker_observability: Vec<WorkerBriefObservability>,
}

/// Map a materialized channel to its localized delegate progress fact.
pub(crate) fn delegate_fact(channel: Channel, brief: &TaskBrief) -> agent_loop::progress::WorkFact {
    let kind = match channel {
        Channel::Rag => agent_loop::progress::ProgressKind::DelegateRag,
        Channel::Search => agent_loop::progress::ProgressKind::DelegateSearch,
    };
    agent_loop::progress::WorkFact::delegate(kind, &brief.goal)
}

pub(crate) struct ChannelOutcome {
    pub record: DispatchRecord,
    pub note: ChannelNote,
    pub observability: Option<WorkerBriefObservability>,
}

/// Run one channel dispatch: session brief → store insert → ledger entry.
///
/// W1: the brief goes to the channel's persistent [`WorkerSession`] (created
/// on first use; a failed session is dropped and replaced — failure
/// isolation). A sealed channel (budget exhausted) yields an Error record
/// carrying the seal signal instead of running.
pub(crate) async fn dispatch_channel(
    channel: Channel,
    query: &str,
    base_request: &AgentRequest,
    executor: &dyn OrchestratorExecutor,
    store: &mut EvidenceStore,
    sink: &dyn AgentEventSink,
    sessions: &mut std::collections::HashMap<Channel, WorkerSession>,
) -> ChannelOutcome {
    let brief = default_brief(channel, query);
    agent_loop::progress::emit_work_fact(sink, delegate_fact(channel, &brief)).await;

    // Failure isolation: poisoned sessions never run again.
    if sessions.get(&channel).is_some_and(|s| s.failed) {
        tracing::warn!(
            channel = channel.as_str(),
            "worker session failed previously; isolating and creating a fresh session"
        );
        sessions.insert(channel, WorkerSession::new(channel));
    }
    let session = sessions
        .entry(channel)
        .or_insert_with(|| WorkerSession::new(channel));

    let dispatch_id = uuid::Uuid::new_v4().to_string();
    match session.run_brief(&brief, base_request, executor).await {
        Ok(Ok(outcome)) => {
            let run = &outcome.run;
            let inserted =
                store.insert_from_tool_results_for_brief(channel, &outcome.tool_results_delta, Some(outcome.seq));
            let failures = tool_failures(&outcome.tool_results_delta);
            let (status, error) = if inserted > 0 {
                if !failures.is_empty() {
                    tracing::warn!(
                        channel = channel.as_str(),
                        failures = ?failures,
                        "orchestrator dispatch partial tool failures"
                    );
                }
                (PackStatus::Ok, None)
            } else if !failures.is_empty() {
                // Retrieval itself failed (e.g. network/tool error) — NOT 未命中.
                (PackStatus::Error, Some(failures.join("; ")))
            } else {
                (PackStatus::Empty, None)
            };
            tracing::info!(
                channel = channel.as_str(),
                status = ?status,
                item_count = inserted,
                brief_seq = outcome.seq,
                worker_tools = ?run.tool_results.iter().map(|t| t.tool.as_str()).collect::<Vec<_>>(),
                "orchestrator dispatch finished"
            );
            // S5: soft fact verification (default OFF) — after the compile,
            // before the note/store finalize; never blocks on failure.
            let mut handoff = outcome.handoff.clone();
            if let Some(h) = handoff.as_mut() {
                super::fact_verify::verify_handoff_facts(h, &outcome.tool_results_delta)
                    .await;
            }
            // K2/W1: hydrate the brief's SELECTED retrieval log (offset
            // applied by the session) into the store's ★ selected tier.
            store.mark_selected_for_brief(channel, &outcome.hydrated, Some(outcome.seq));
            let handoff_degraded = handoff
                .as_ref()
                .map(|h| h.handoff_degraded)
                .unwrap_or(false);
            ChannelOutcome {
                record: DispatchRecord {
                    channel,
                    dispatch_id,
                    status,
                    item_count: inserted,
                    error: error.clone(),
                },
                note: ChannelNote::with_handoff(
                    channel,
                    status,
                    inserted,
                    handoff,
                    error,
                ),
                observability: Some(WorkerBriefObservability {
                    channel,
                    seq: outcome.seq,
                    handoff_degraded,
                    run: worker_observability_from_run(channel, run),
                }),
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(
                channel = channel.as_str(),
                error = %e,
                "orchestrator dispatch failed"
            );
            ChannelOutcome {
                record: DispatchRecord {
                    channel,
                    dispatch_id,
                    status: PackStatus::Error,
                    item_count: 0,
                    error: Some(e.to_string()),
                },
                note: ChannelNote::with_handoff(
                    channel,
                    PackStatus::Error,
                    0,
                    None,
                    Some(e.to_string()),
                ),
                observability: None,
            }
        }
        Err(SessionError::BudgetExhausted) => {
            // W2: channel cap spent — seal signal on the record.
            let msg = "channel budget exhausted".to_string();
            tracing::info!(channel = channel.as_str(), "{msg}");
            ChannelOutcome {
                record: DispatchRecord {
                    channel,
                    dispatch_id,
                    status: PackStatus::Error,
                    item_count: 0,
                    error: Some(msg.clone()),
                },
                note: ChannelNote::with_handoff(
                    channel,
                    PackStatus::Error,
                    0,
                    None,
                    Some(msg),
                ),
                observability: None,
            }
        }
    }
}

/// Run orchestrated turn: materialize → workers → chat.
pub async fn run_orchestrated_turn(
    caps: CapabilitySet,
    base_request: &AgentRequest,
    executor: &dyn OrchestratorExecutor,
    sink: &dyn AgentEventSink,
    docscope: Option<&common::DocScopeMetadata>,
) -> Result<OrchestratedTurn, AppError> {
    let label = caps.agent_type_label().to_string();
    let channels = materialize_channels(caps);
    let query = base_request.query.clone();

    // Pure chat: no workers
    if channels.is_empty() {
        let handoff = direct_handoff(&query);
        agent_loop::progress::emit_work_fact(
            sink,
            agent_loop::progress::WorkFact::understand(&query),
        )
        .await;
        let answer_result = executor.run_chat(&handoff, base_request, sink).await?;
        return Ok(OrchestratedTurn {
            answer_result,
            store: EvidenceStore::from_docscope(docscope),
            records: vec![],
            handoff,
            agent_type_label: label,
            worker_observability: vec![],
        });
    }

    let mut store = EvidenceStore::from_docscope(docscope);
    let mut records: Vec<DispatchRecord> = Vec::new();
    let mut channel_notes: Vec<ChannelNote> = Vec::new();
    let mut worker_observability: Vec<WorkerBriefObservability> = Vec::new();
    // W1: per-channel persistent worker sessions (one turn lifetime). V1's
    // single default brief per channel is exactly one brief per session —
    // byte-identical to the pre-W1 path.
    let mut sessions: std::collections::HashMap<Channel, WorkerSession> =
        std::collections::HashMap::new();

    // Turn-start fact (2026-07-23): the first dispatch decision can take tens
    // of seconds — give the client an immediate progress step, not dead air.
    agent_loop::progress::emit_work_fact(sink, agent_loop::progress::WorkFact::understand(&query))
        .await;

    // §7.1 first wave: every materialized channel
    for ch in &channels {
        let outcome = dispatch_channel(
            *ch,
            &query,
            base_request,
            executor,
            &mut store,
            sink,
            &mut sessions,
        )
        .await;
        records.push(outcome.record);
        channel_notes.push(outcome.note);
        if let Some(obs) = outcome.observability {
            worker_observability.push(obs);
        }
    }

    // §7.2 assert. The first wave above always pushes a record per materialized
    // channel (even on worker error), so this recovery branch is unreachable
    // today — it is kept as defense for the O2 LLM-dispatch path, where the
    // orchestrator may skip a channel and the invariant must force a default run.
    if let Err(missing) = assert_complete(&channels, &records) {
        for ch in missing.channels {
            let outcome = dispatch_channel(
                ch,
                &query,
                base_request,
                executor,
                &mut store,
                sink,
                &mut sessions,
            )
            .await;
            channel_notes.push(outcome.note);
            if let Some(obs) = outcome.observability {
                worker_observability.push(obs);
            }
        }
    }
    assert_complete(&channels, &records).map_err(|m| {
        AppError::internal(format!("orchestrator completion invariant failed: {m}"))
    })?;

    let handoff = synthesize_handoff(
        &query,
        store.source_docs().to_vec(),
        store.listings(),
        store.targeted_entries(),
        channel_notes,
        &records,
        None,
    );
    agent_loop::progress::emit_work_fact(
        sink,
        agent_loop::progress::WorkFact::compose_answer(),
    )
    .await;

    let mut answer_result = executor.run_chat(&handoff, base_request, sink).await?;
    // Single point where E-markers become product markers + citations; dangling
    // or fabricated markers are stripped here. Streamed tokens may briefly show
    // E-ids; the terminal `done` payload carries the rewritten answer.
    finalize_answer_evidence(&mut answer_result, &store);

    Ok(OrchestratedTurn {
        answer_result,
        store,
        records,
        handoff,
        agent_type_label: label,
        worker_observability,
    })
}

/// §0.1 answer-rule blocks chosen by the materials this handoff actually
/// carries (workspace doc evidence / web evidence) — not by what the user
/// checked. DocProfile listings are orientation context, not citable material.
/// (P1-1: the follow-brief layer merged into `product-answer-base`; these are
/// the per-material blocks only.)
fn answer_rule_parts(handoff: &ChatHandoff) -> Vec<String> {
    let has_doc = handoff
        .listings
        .iter()
        .any(|l| l.channel == Channel::Rag && l.kind != EvidenceKind::DocProfile);
    let has_web = handoff
        .listings
        .iter()
        .any(|l| l.channel == Channel::Search);
    let mut parts = Vec::new();
    // P3: answer-from-workspace carries the core grounding rules (文档事实只能
    // 来自证据 / 不得用常识补写) that zero-evidence turns need most — inject
    // unconditionally (previously gated on has_doc).
    parts.push("prompts/deprecated/orchestrator-multiagent/answer-from-workspace.md".to_string());
    if has_web {
        parts.push("prompts/deprecated/orchestrator-multiagent/answer-from-web.md".to_string());
    }
    if has_doc && has_web {
        parts.push("prompts/deprecated/orchestrator-multiagent/answer-dual-source.md".to_string());
    }
    parts
}

/// Production executor: runs single-channel / chat via UnifiedAgentService.
pub struct AgentServiceExecutor {
    pub agent_service: std::sync::Arc<crate::agents::service::UnifiedAgentService>,
    /// Client-facing sink for worker progress fan-out (2026-07-23: workers
    /// used to run fully silent during Dispatch). `None` keeps workers silent
    /// (non-streaming turns and tests).
    pub progress_sink: Option<Box<dyn agent_loop::events::AgentEventSink>>,
}

impl AgentServiceExecutor {
    pub fn new(agent_service: std::sync::Arc<crate::agents::service::UnifiedAgentService>) -> Self {
        Self {
            agent_service,
            progress_sink: None,
        }
    }

    pub fn with_progress_sink(
        agent_service: std::sync::Arc<crate::agents::service::UnifiedAgentService>,
        progress_sink: Box<dyn agent_loop::events::AgentEventSink>,
    ) -> Self {
        Self {
            agent_service,
            progress_sink: Some(progress_sink),
        }
    }
}

#[async_trait]
impl OrchestratorExecutor for AgentServiceExecutor {
    async fn run_channel(
        &self,
        channel: Channel,
        brief: &TaskBrief,
        base: &AgentRequest,
    ) -> Result<AgentRunResult, AppError> {
        let mut req = base.clone();
        // The worker's query IS the brief goal: V1 default briefs are the
        // policy-free passthrough of the user query (no change), and V2 LLM
        // briefs are self-contained + de-referenced, so retrieval / injection
        // runs on the orchestrator's words rather than the raw utterance.
        req.query = brief.goal.clone();
        req.kind = match channel {
            Channel::Rag => crate::agents::AgentKind::Rag,
            Channel::Search => crate::agents::AgentKind::Search,
        };
        // Single-channel capability metadata so assembly path stays pure if used
        let caps = match channel {
            Channel::Rag => serde_json::json!(["rag"]),
            Channel::Search => serde_json::json!(["search"]),
        };
        req.metadata.insert("capabilities".into(), caps);
        // Prefer loading mode via kind; strip dual assembled config so worker is pure
        req.metadata.remove("assembled_mode_config");
        req.metadata.remove("system_prompt_parts");
        // Inject single-channel assembled config
        let cap_set = match channel {
            Channel::Rag => CapabilitySet {
                rag: true,
                search: false,
            },
            Channel::Search => CapabilitySet {
                rag: false,
                search: true,
            },
        };
        if let Ok(assembled) = crate::assemble_mode(cap_set) {
            let mut parts = assembled.system_prompt_parts;
            // U12: the evidence-pointer rule varies by channel — rag workers
            // ground on code-execution observations; search workers have no
            // codegen and ground on native tool results.
            let evidence_rule = match channel {
                Channel::Rag => "evidence pointers must come from your **code-execution observations** \
                 (`<code_execution_result>` / retrieval chunks), not from inventing native \
                 tool calls. Workspace retrieval is `<code language=\"python\">` + \
                 `await client.…` only.",
                Channel::Search => "evidence pointers must come from your **native tool results** \
                 (`web_search` / `web_fetch` observations), not from inventing tool calls.",
            };
            // E2: a "not found / insufficient" verdict must rest on REAL
            // retrieval calls this run — the tool vocabulary differs by
            // channel (this brief only ever reaches rag/search workers).
            let retrieval_proof_rule = match channel {
                Channel::Rag => "查无 / coverage=insufficient 的结论必须以本轮**真实检索调用记录**为支撑\
                 （dense/lexical/graph/doc_scan 至少执行过）；零检索调用得出的『未找到』不可接受\
                 ——先检索，再谈覆盖。",
                Channel::Search => "查无 / coverage=insufficient 的结论必须以本轮**真实检索调用记录**为支撑\
                 （web_search/web_fetch 至少执行过）；零检索调用得出的『未找到』不可接受\
                 ——先检索，再谈覆盖。",
            };
            // Brief + worker-output slim: the worker's final message is an
            // internal hand-off; the chat exit writes the user answer.
            // K3: the handoff contract is prose + optional SELECTED line —
            // JSON is still accepted for structured fields, key_facts and
            // hand-copied evidence pointers are gone (code hydrates them).
            parts.push(format!(
                "## Task brief (orchestrator)\n{}\n\n\
                 Execute only this brief. Your final message is an **internal hand-off**, \
                 not the user-facing answer — another agent writes the user answer.\n\n\
                 最终消息 = **分析散文**（写清发现了什么 / 没发现什么、覆盖判断）；\
                 也可以输出 internal_worker_handoff_v1 JSON（summary / coverage / gaps，\
                 可选 premise_mismatch）。**不要**代码块或 markdown 围栏包装。\n\
                 收尾时：凡实际用到的证据，**另起一行**写 `SELECTED: #n, #m`——\
                 检索结果 dict 自带 `alias` 字段，只列真正用到的编号；没用到就不写这一行。\
                 **不要抄 chunk UUID，不要用描述代替编号**（指针由系统按编号水合，无需你提供）。\n\
                 Rules:\n\
                 - 覆盖判断要诚实：找到什么写什么，没覆盖的维度明确说未覆盖；{evidence_rule}\n\
                 - `premise_mismatch`（仅 JSON 形式时）：若问题的框架/主体归属/口径与证据不符，\
                 在 JSON 里以该字段上报并写明 `actual_subject`（kind 可为 entity|frame|scope|definition——\
                 文档中有候选证据但口径存疑时，如「第一阶段…按4A架构详细设计」vs「详细设计阶段」，\
                 不得替用户裁决，附上候选日期/原文）；散文形式时在文中说明即可。\n\
                 - 查无即成功: when the evidence genuinely does not cover the question, \
                 `coverage=insufficient` + `gaps` explaining what is absent IS a complete \
                 successful delivery, not a failure.\n\
                 - 查无须凭据: {retrieval_proof_rule}",
                brief.goal
            ));
            req.metadata.insert(
                "assembled_mode_config".into(),
                serde_json::to_value(&assembled.config).unwrap_or(serde_json::json!({})),
            );
            req.metadata.insert(
                "system_prompt_parts".into(),
                serde_json::to_value(&parts).unwrap_or(serde_json::json!([])),
            );
        }
        req.stream = false;
        let local = CollectingSink::new();
        // Fan out worker retrieval progress (Activity only) to the client
        // stream when a progress sink is wired; worker text stays local.
        let mut run = if let Some(progress) = self.progress_sink.as_deref() {
            let tee = agent_loop::events::ProgressTeeSink::new(
                local.clone_boxed(),
                progress.clone_boxed(),
            );
            self.agent_service.run(req, &tee).await?
        } else {
            self.agent_service.run(req, &local).await?
        };
        // Sub-agent thinking (plan/eval/codegen) lives on the local sink only —
        // ProgressTee does not forward it to the client. Attach for mode_debug.
        attach_worker_thinking_events(&mut run, &local.events());
        Ok(run)
    }

    async fn run_chat(
        &self,
        handoff: &ChatHandoff,
        base: &AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let mut req = base.clone();
        req.query = query_for_agent(handoff);
        req.kind = crate::agents::AgentKind::Chat;
        req.metadata.insert(
            "capabilities".into(),
            serde_json::json!([]),
        );
        req.metadata.remove("assembled_mode_config");

        // Option D Answer pack (P1-2): product-answer-base + material blocks.
        // product-answer-base carries voice + memory protocol + grounding rules;
        // full chat-base is pure-chat only (its "你不执行检索" narrative fueled
        // refusal rhetoric in the Answer phase — full_eval Q129).
        // Custom ModeConfig keeps prose_only contract and adds utility tools.
        // Material answer-* blocks only for synthesize (has citable materials);
        // mode=direct is chat-like (no evidence pack, no workspace/web blocks).
        if let Ok(assembled) = crate::assemble_mode(CapabilitySet::default()) {
            let mut parts = vec!["prompts/deprecated/orchestrator-multiagent/product-answer-base.md".to_string()];
            if handoff.mode == super::types::ChatExitMode::Synthesize {
                parts.extend(answer_rule_parts(handoff));
            }

            let mut answer_config = assembled.config;
            // Utility tools whitelist (OQ-Tools): same pool as AnswerOnly pure
            // chat. Memory stays orthogonal via skill disclosure.
            answer_config.tool_pool = crate::mode_assemble::utility_tool_pool();
            // Answer phase: evidence already finalized by Dispatch; no retrieval required.
            answer_config.loop_exit.require_evidence = false;
            answer_config.loop_exit.allow_content_early_stop = true;
            answer_config.loop_exit.skip_synthesis_on_direct_answer = true;
            answer_config.inject_retrieval_query = false;
            answer_config.auto_fallback = None;
            // Keep prose_only contract (no JSON envelope for user-facing answer).
            answer_config.synthesis_output.contract = agent_loop::r#loop::config::AnswerContractKind::ProseOnly;
            // P0-2 / PR-A: do not inherit chat.yaml (or future) mandatory synthesis
            // — product-answer-base is the Answer voice; stacking synthesis/chat.md
            // after utility tools reopened dual persona.
            answer_config.skill_catalog.mandatory.synthesis.clear();

            req.metadata.insert(
                "assembled_mode_config".into(),
                serde_json::to_value(&answer_config).unwrap_or(serde_json::json!({})),
            );
            req.metadata.insert(
                "system_prompt_parts".into(),
                serde_json::to_value(&parts)
                    .unwrap_or(serde_json::json!([])),
            );
        }
        // Env-gated debug dump of the outgoing Answer-phase request (query +
        // parts). For diagnosing "evidence present but not cited" cases —
        // writes one JSON line per Answer run; dev-only, off by default.
        if std::env::var("OPTION_D_ANSWER_DEBUG_DUMP").is_ok() {
            let dump = serde_json::json!({
                "query_char_len": req.query.chars().count(),
                "system_prompt_parts": req.metadata.get("system_prompt_parts"),
                "query": req.query,
            });
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/answer_pack_debug.jsonl")
            {
                use std::io::Write;
                let _ = writeln!(f, "{dump}");
            }
        }
        // Live tokens go to the orchestrator sink (SSE when streaming). Workers
        // still use a private CollectingSink — only the chat exit is user-facing.
        req.stream = base.stream;
        self.agent_service.run(req, sink).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::PackStatus;
    use agent_loop::runtime::AgentRequest;
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
    use std::sync::Mutex;
    use uuid::Uuid;

    struct MockExec {
        channels_run: Mutex<Vec<Channel>>,
    }

    #[async_trait]
    impl OrchestratorExecutor for MockExec {
        async fn run_channel(
            &self,
            channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            self.channels_run.lock().unwrap().push(channel);
            let mut r = AgentRunResult::default();
            if channel == Channel::Search {
                r.tool_results = vec![contracts::ToolResult {
                    tool: "web_search".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!({
                        "results": [{"url": "https://a.com", "title": "A", "snippet": "best practice"}]
                    })),
                    trace: None,
                }];
            }
            Ok(r)
        }

        async fn run_chat(
            &self,
            handoff: &ChatHandoff,
            _base: &AgentRequest,
            _sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = if handoff.partial_notices.is_empty() {
                "full answer".into()
            } else {
                format!(
                    "partial: {} | {}",
                    handoff.partial_notices.join("; "),
                    "工作区未命中相关段落；以下基于网页。"
                )
            };
            Ok(r)
        }
    }

    fn base_req(q: &str) -> AgentRequest {
        AgentRequest {
            kind: crate::agents::AgentKind::Chat,
            query: q.into(),
            workspace_id: None,
            session_id: None,
            doc_scope: vec!["doc1".into()],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth: AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::User),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
        }
    }

    #[tokio::test]
    async fn pure_chat_skips_workers() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(CapabilitySet::default(), &base_req("hi"), &ex, &sink, None)
            .await
            .unwrap();
        assert!(ex.channels_run.lock().unwrap().is_empty());
        assert!(turn.records.is_empty());
        assert_eq!(turn.agent_type_label, "chat");
    }

    #[tokio::test]
    async fn dual_runs_both_channels() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let turn = run_orchestrated_turn(caps, &base_req("报告与最佳实践差距"), &ex, &sink, None)
            .await
            .unwrap();
        let ran = ex.channels_run.lock().unwrap().clone();
        assert!(ran.contains(&Channel::Rag));
        assert!(ran.contains(&Channel::Search));
        assert_eq!(turn.records.len(), 2);
        // rag empty → hard zero-evidence notice (P3 wording)
        assert!(
            turn.handoff.partial_notices.iter().any(|n| n.contains("未检索到任何证据")),
            "notices={:?}",
            turn.handoff.partial_notices
        );
        // search returned one web entry → it is in the store + handoff listings
        assert_eq!(turn.store.count_channel(Channel::Search), 1);
        assert!(turn.handoff.listings.iter().any(|l| l.channel == Channel::Search));
        assert!(!crate::orchestrator::invariant::looks_like_user_did_not_provide_doc(
            &turn.answer_result.answer
        ));
    }

    #[tokio::test]
    async fn rag_only_empty_records_empty_status() {
        let ex = MockExec {
            channels_run: Mutex::new(vec![]),
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(
            CapabilitySet {
                rag: true,
                search: false,
            },
            &base_req("总结文档"),
            &ex,
            &sink,
            None,
        )
        .await
        .unwrap();
        assert_eq!(*ex.channels_run.lock().unwrap(), vec![Channel::Rag]);
        assert_eq!(turn.records[0].status, PackStatus::Empty);
        assert_eq!(turn.records[0].item_count, 0);
    }

    #[tokio::test]
    async fn citations_rebuilt_from_store_eids() {
        struct CiteMockExec;
        #[async_trait]
        impl OrchestratorExecutor for CiteMockExec {
            async fn run_channel(
                &self,
                channel: Channel,
                _brief: &TaskBrief,
                _base: &AgentRequest,
            ) -> Result<AgentRunResult, AppError> {
                let mut r = AgentRunResult::default();
                r.tool_results = vec![match channel {
                    Channel::Rag => contracts::ToolResult {
                        tool: "dense_retrieval".into(),
                        version: "1".into(),
                        status: contracts::ToolStatus::Ok,
                        data: Some(serde_json::json!([
                            {"chunk_id": "chunk-a", "doc_id": "doc1", "text": "doc evidence", "score": 0.9, "page": 4}
                        ])),
                        trace: None,
                    },
                    Channel::Search => contracts::ToolResult {
                        tool: "web_search".into(),
                        version: "1".into(),
                        status: contracts::ToolStatus::Ok,
                        data: Some(serde_json::json!({
                            "results": [
                                {"url": "https://a.example", "title": "A", "snippet": "web evidence"}
                            ]
                        })),
                        trace: None,
                    },
                }];
                Ok(r)
            }

            async fn run_chat(
                &self,
                _handoff: &ChatHandoff,
                _base: &AgentRequest,
                _sink: &dyn AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                let mut r = AgentRunResult::default();
                r.answer = "文档证据 [[E1]]，网页佐证 [[E2]]。编造 [[E9]]。".into();
                Ok(r)
            }
        }

        let caps = CapabilitySet {
            rag: true,
            search: true,
        };
        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(caps, &base_req("对比"), &CiteMockExec, &sink, None)
            .await
            .unwrap();
        // Valid E-ids became product markers; fabricated E9 stripped.
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
        assert!(turn.answer_result.answer.contains("[[web:2]]"));
        assert!(!turn.answer_result.answer.contains("[[E"));
        assert_eq!(turn.answer_result.citations.len(), 2);
        assert!(
            turn.answer_result
                .citations
                .iter()
                .any(|c| c.chunk_id.as_deref() == Some("chunk-a") && c.page == Some(4))
        );
        assert!(
            turn.answer_result
                .citations
                .iter()
                .any(|c| c.layer.as_deref() == Some("search") && c.doc_id == "https://a.example")
        );
        assert_eq!(turn.answer_result.sources.len(), 1);
    }

    #[tokio::test]
    async fn tool_failure_marks_channel_error_not_empty() {
        struct FailExec;
        #[async_trait]
        impl OrchestratorExecutor for FailExec {
            async fn run_channel(
                &self,
                _channel: Channel,
                _brief: &TaskBrief,
                _base: &AgentRequest,
            ) -> Result<AgentRunResult, AppError> {
                let mut r = AgentRunResult::default();
                r.tool_results = vec![contracts::ToolResult {
                    tool: "web_search".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Timeout,
                    data: Some(serde_json::json!({"error": "dns poisoned"})),
                    trace: None,
                }];
                Ok(r)
            }

            async fn run_chat(
                &self,
                _handoff: &ChatHandoff,
                _base: &AgentRequest,
                _sink: &dyn AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                Ok(AgentRunResult::default())
            }
        }

        let sink = CollectingSink::new();
        let turn = run_orchestrated_turn(
            CapabilitySet {
                rag: true,
                search: false,
            },
            &base_req("q"),
            &FailExec,
            &sink,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records[0].status, PackStatus::Error);
        assert!(
            turn.records[0]
                .error
                .as_deref()
                .unwrap_or("")
                .contains("Timeout")
        );
        // Notice must say 检索失败, not 未命中.
        assert!(
            turn
                .handoff
                .partial_notices
                .iter()
                .any(|n| n.contains("检索失败")),
            "notices: {:?}",
            turn.handoff.partial_notices
        );
        assert!(
            !turn
                .handoff
                .partial_notices
                .iter()
                .any(|n| n.contains("未命中")),
            "notices: {:?}",
            turn.handoff.partial_notices
        );
    }

    #[tokio::test]
    async fn worker_query_is_brief_goal_and_brief_reaches_prompt_parts() {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor::new(svc);
        exec.run_channel(
            Channel::Rag,
            &TaskBrief::new("brief goal text"),
            &base_req("用户原始问题"),
        )
        .await
        .unwrap();

        let req = captured.lock().unwrap().clone().expect("captured request");
        // Worker query = the (self-contained) brief goal, not the raw utterance.
        assert_eq!(req.query, "brief goal text");
        let parts = req
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            parts
                .iter()
                .filter_map(|p| p.as_str())
                .any(|s| {
                    s.contains("brief goal text")
                        && s.contains("internal hand-off")
                        && s.contains("internal_worker_handoff_v1")
                        && s.contains("code-execution observations")
                        && s.contains("await client.")
                }),
            "brief part missing or still says tool-results-only: {parts:?}"
        );
    }

    fn listing(eid: &str, channel: Channel, kind: EvidenceKind) -> super::super::store::EvidenceListing {
        super::super::store::EvidenceListing {
            eid: eid.into(),
            channel,
            kind,
            label: "label".into(),
            preview: "preview".into(),
            full_text: "full body".into(),
            chunk_id: None,
            doc_id: None,
            score: None,
            url: None,
            selected: false,
        }
    }

    #[test]
    fn answer_rule_parts_follow_actual_materials() {
        // P3: answer-from-workspace is UNCONDITIONAL (grounding rules matter
        // most when evidence is empty) — every handoff carries it first.
        let h = direct_handoff("q");
        assert_eq!(
            answer_rule_parts(&h),
            vec!["prompts/deprecated/orchestrator-multiagent/answer-from-workspace.md".to_string()]
        );

        // DocProfile only = orientation, not material — workspace block stays
        // unconditional; no web/dual block.
        let mut h = direct_handoff("q");
        h.listings = vec![listing("E1", Channel::Rag, EvidenceKind::DocProfile)];
        assert_eq!(
            answer_rule_parts(&h),
            vec!["prompts/deprecated/orchestrator-multiagent/answer-from-workspace.md".to_string()]
        );

        // Workspace only.
        let mut h = direct_handoff("q");
        h.listings = vec![listing("E1", Channel::Rag, EvidenceKind::DocChunk)];
        assert_eq!(
            answer_rule_parts(&h),
            vec!["prompts/deprecated/orchestrator-multiagent/answer-from-workspace.md".to_string()]
        );

        // Web only: workspace (unconditional) + web.
        let mut h = direct_handoff("q");
        h.listings = vec![listing("E1", Channel::Search, EvidenceKind::WebPage)];
        assert_eq!(
            answer_rule_parts(&h),
            vec![
                "prompts/deprecated/orchestrator-multiagent/answer-from-workspace.md".to_string(),
                "prompts/deprecated/orchestrator-multiagent/answer-from-web.md".to_string()
            ]
        );

        // Dual: both blocks + dual-source comparator.
        let mut h = direct_handoff("q");
        h.listings = vec![
            listing("E1", Channel::Rag, EvidenceKind::DocChunk),
            listing("E2", Channel::Search, EvidenceKind::WebPage),
        ];
        let parts = answer_rule_parts(&h);
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts.iter().any(|p| p.contains("answer-dual-source")));
    }

    #[tokio::test]
    async fn chat_exit_system_parts_include_material_blocks() {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor::new(svc);

        // Material blocks only apply on synthesize (not mode=direct).
        let handoff = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag, EvidenceKind::DocChunk)],
            vec![],
            vec![],
            &[],
            None,
        );
        exec.run_chat(&handoff, &base_req("q"), &CollectingSink::new())
            .await
            .unwrap();

        let req = captured.lock().unwrap().clone().expect("captured request");
        let parts: Vec<String> = req
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "{parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("chat-base.md")),
            "P1-2: full chat-base must not load in Answer pack: {parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("answer-from-workspace.md")),
            "{parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("answer-from-web.md")),
            "web block must not load for workspace-only materials: {parts:?}"
        );
    }

    #[tokio::test]
    async fn answer_pack_system_parts_include_product_answer_base() {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor::new(svc);

        let handoff = direct_handoff("q");
        exec.run_chat(&handoff, &base_req("q"), &CollectingSink::new())
            .await
            .unwrap();

        let req = captured.lock().unwrap().clone().expect("captured request");
        let parts: Vec<String> = req
            .metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "product-answer-base must be first part: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("chat-base.md")),
            "P1-2: full chat-base must not load in Answer pack: {parts:?}"
        );
        // No evidence → no answer-* blocks.
        assert!(
            !parts.iter().any(|p| p.contains("answer-from-workspace.md")),
            "no material blocks for empty handoff: {parts:?}"
        );
    }

    #[tokio::test]
    async fn answer_pack_mode_config_has_utility_tools_and_prose_contract() {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor::new(svc);

        let handoff = direct_handoff("q");
        exec.run_chat(&handoff, &base_req("q"), &CollectingSink::new())
            .await
            .unwrap();

        let req = captured.lock().unwrap().clone().expect("captured request");
        let config: agent_loop::r#loop::config::ModeConfig = serde_json::from_value(
            req.metadata
                .get("assembled_mode_config")
                .cloned()
                .unwrap_or_default(),
        )
        .expect("mode config deserialized");

        assert_eq!(config.synthesis_output.contract, agent_loop::r#loop::config::AnswerContractKind::ProseOnly);
        assert!(!config.loop_exit.require_evidence);
        assert!(config.loop_exit.allow_content_early_stop);
        assert!(config.loop_exit.skip_synthesis_on_direct_answer);
        assert!(
            config.skill_catalog.mandatory.synthesis.is_empty(),
            "Answer must not inherit mandatory synthesis/chat: {:?}",
            config.skill_catalog.mandatory.synthesis
        );
        assert!(config.tool_pool.contains(&"user_context".to_string()));
        assert!(config.tool_pool.contains(&"calculator".to_string()));
        assert!(config.tool_pool.contains(&"weather_query".to_string()));
        assert!(!config.tool_pool.contains(&"dense_retrieval".to_string()));
        assert!(!config.tool_pool.contains(&"delegate_rag".to_string()));
        assert!(!config.tool_pool.contains(&"delegate_search".to_string()));
    }

    /// 2026-07-23 UX: with a progress sink wired, run_channel fans out only
    /// Activity (progress) — worker prose stays out of the client stream.
    #[tokio::test]
    async fn run_channel_fans_out_only_activity_to_progress_sink() {
        use agent_loop::runtime::Agent;
        use crate::orchestrator::types::TaskBrief;

        struct NoisyAgent;
        #[async_trait]
        impl Agent for NoisyAgent {
            async fn run(
                &self,
                _request: AgentRequest,
                sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                sink.emit(agent_loop::events::AgentEvent::Activity {
                    stage: "act:search_web".into(),
                    message: "progress.search_web.running".into(),
                    detail: None,
                    counts: Default::default(),
                    sources_preview: vec![],
                })
                .await
                .ok();
                sink.emit(agent_loop::events::AgentEvent::MessageDelta {
                    text: "worker internal text".into(),
                })
                .await
                .ok();
                Ok(AgentRunResult::default())
            }
        }

        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(NoisyAgent),
        ));
        let progress = CollectingSink::new();
        let exec = AgentServiceExecutor::with_progress_sink(svc, progress.clone_boxed());
        let brief = TaskBrief::new("q");
        exec.run_channel(Channel::Search, &brief, &base_req("q"))
            .await
            .unwrap();

        let events = progress.events();
        assert_eq!(events.len(), 1, "only Activity may reach the client: {events:?}");
        assert!(matches!(
            &events[0],
            agent_loop::events::AgentEvent::Activity { .. }
        ));
    }

    /// PR-A: worker ModeConfig is ProseOnly + early-stop (handoff final), not unified JSON.
    #[tokio::test]
    async fn worker_channel_uses_handoff_prose_only_contract() {
        use agent_loop::runtime::Agent;
        use crate::orchestrator::types::TaskBrief;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        let exec = AgentServiceExecutor::new(svc);
        let brief = TaskBrief::new("extract IPO price");
        exec.run_channel(Channel::Rag, &brief, &base_req("q"))
            .await
            .unwrap();
        let req = captured.lock().unwrap().clone().expect("captured");
        let config: agent_loop::r#loop::config::ModeConfig = serde_json::from_value(
            req.metadata
                .get("assembled_mode_config")
                .cloned()
                .unwrap_or_default(),
        )
        .expect("mode config");
        assert_eq!(
            config.synthesis_output.contract,
            agent_loop::r#loop::config::AnswerContractKind::ProseOnly
        );
        assert!(
            !config.loop_exit.require_evidence,
            "require_evidence is skill-owned, not host-forced"
        );
        assert!(!config.loop_exit.allow_content_early_stop);
        assert!(config.loop_exit.skip_synthesis_on_direct_answer);
        assert!(config.skill_catalog.mandatory.synthesis.is_empty());
        let parts = parts_of(&req);
        assert!(
            parts.iter().any(|p| p.contains("internal_worker_handoff_v1")),
            "worker brief must still inject handoff schema: {parts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Option D T1–T2: evidence slot, direct, dual blocks, phrase mutex, tools
    // -----------------------------------------------------------------------

    fn parts_of(req: &AgentRequest) -> Vec<String> {
        req.metadata
            .get("system_prompt_parts")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn capture_exec() -> (
        std::sync::Arc<Mutex<Option<AgentRequest>>>,
        AgentServiceExecutor,
    ) {
        use agent_loop::runtime::Agent;

        struct CaptureAgent(std::sync::Arc<Mutex<Option<AgentRequest>>>);
        #[async_trait]
        impl Agent for CaptureAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn agent_loop::events::AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                *self.0.lock().unwrap() = Some(request);
                Ok(AgentRunResult::default())
            }
        }

        let captured = std::sync::Arc::new(Mutex::new(None));
        let svc = std::sync::Arc::new(crate::agents::service::UnifiedAgentService::new(
            Box::new(CaptureAgent(captured.clone())),
        ));
        (captured, AgentServiceExecutor::new(svc))
    }

    /// G-01 (KD-16): Evidence body lives in **query**; system has no `### Evidence`.
    #[tokio::test]
    async fn answer_pack_evidence_lives_in_query_not_system() {
        let (captured, exec) = capture_exec();
        const BODY: &str = "UNIQUE_EVIDENCE_BODY_G01_full_chunk_text";
        let mut listing = listing("E1", Channel::Rag, EvidenceKind::DocChunk);
        listing.full_text = BODY.into();
        let handoff = synthesize_handoff(
            "报告写了什么",
            vec![],
            vec![listing],
            vec![],
            vec![],
            &[],
            Some("按文档摘要".into()),
        );
        exec.run_chat(&handoff, &base_req("报告写了什么"), &CollectingSink::new())
            .await
            .unwrap();
        let req = captured.lock().unwrap().clone().expect("captured");
        assert!(
            req.query.contains("### Evidence"),
            "synthesize query must carry Evidence section: {}",
            req.query.chars().take(200).collect::<String>()
        );
        assert!(
            req.query.contains(BODY),
            "query must include full_text chunk body"
        );
        let parts = parts_of(&req);
        let system_blob = parts.join("\n");
        assert!(
            !system_blob.contains("### Evidence"),
            "system_prompt_parts must not embed Evidence section: {parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "{parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("answer-from-workspace.md")),
            "{parts:?}"
        );
    }

    /// G-03: mode=direct → no Evidence in query, no material answer blocks.
    #[tokio::test]
    async fn answer_pack_direct_mode_skips_evidence_and_material_blocks() {
        let (captured, exec) = capture_exec();
        // Even if listings were present, Direct uses user_query only (query_for_agent).
        let mut handoff = direct_handoff("随便聊聊");
        handoff.listings = vec![listing("E1", Channel::Rag, EvidenceKind::DocChunk)];
        exec.run_chat(&handoff, &base_req("随便聊聊"), &CollectingSink::new())
            .await
            .unwrap();
        let req = captured.lock().unwrap().clone().expect("captured");
        assert_eq!(req.query, "随便聊聊");
        assert!(
            !req.query.contains("### Evidence"),
            "direct query must not inject Evidence"
        );
        let parts = parts_of(&req);
        assert!(
            parts.iter().any(|p| p.contains("product-answer-base.md")),
            "{parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("answer-from-")),
            "direct must not inject answer-from-* blocks: {parts:?}"
        );
    }

    /// G-07: dual materials → workspace + web + dual-source blocks on run_chat.
    #[tokio::test]
    async fn answer_pack_dual_materials_load_all_answer_blocks() {
        let (captured, exec) = capture_exec();
        let handoff = synthesize_handoff(
            "对比文档与网页",
            vec![],
            vec![
                listing("E1", Channel::Rag, EvidenceKind::DocChunk),
                listing("E2", Channel::Search, EvidenceKind::WebPage),
            ],
            vec![],
            vec![],
            &[],
            None,
        );
        exec.run_chat(&handoff, &base_req("对比文档与网页"), &CollectingSink::new())
            .await
            .unwrap();
        let req = captured.lock().unwrap().clone().expect("captured");
        let parts = parts_of(&req);
        assert!(
            parts.iter().any(|p| p.contains("answer-from-workspace.md")),
            "{parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("answer-from-web.md")),
            "{parts:?}"
        );
        assert!(
            parts.iter().any(|p| p.contains("answer-dual-source.md")),
            "{parts:?}"
        );
        assert!(req.query.contains("### Evidence"), "dual synthesize has Evidence");
    }

    /// G-08 / KD-17: empty evidence → no answer-* blocks; query carries no-marker contract.
    #[tokio::test]
    async fn answer_pack_empty_evidence_single_source_contract() {
        let (captured, exec) = capture_exec();
        let rec = DispatchRecord {
            channel: Channel::Rag,
            dispatch_id: "d1".into(),
            status: PackStatus::Empty,
            item_count: 0,
            error: None,
        };
        let note = ChannelNote {
            channel: Channel::Rag,
            status: PackStatus::Empty,
            item_count: 0,
            note: None,
            handoff: None,
            error: None,
        };
        let handoff = synthesize_handoff("库里有吗", vec![], vec![], vec![], vec![note], &[rec], None);
        exec.run_chat(&handoff, &base_req("库里有吗"), &CollectingSink::new())
            .await
            .unwrap();
        let req = captured.lock().unwrap().clone().expect("captured");
        let parts = parts_of(&req);
        // P3: grounding rules are unconditional — answer-from-workspace is
        // present even with empty materials (it is what forbids confabulation);
        // the material-gated web/dual blocks stay absent.
        assert!(
            parts.iter().any(|p| p.contains("answer-from-workspace")),
            "workspace grounding block must be unconditional: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("answer-from-web") || p.contains("answer-dual-source")),
            "empty materials → no web/dual blocks: {parts:?}"
        );
        assert!(
            req.query.contains("do not emit any citation markers")
                || req.query.contains("do not emit"),
            "empty synthesize query must state no-marker contract: {}",
            req.query.chars().take(400).collect::<String>()
        );
        // System must not also paste a long duplicate no-evidence essay via parts paths only.
        let system_blob = parts.join("\n");
        assert!(!system_blob.contains("### Evidence"));
    }

    /// G-04: Answer pack prompts must not carry Dispatch “don't write final answer” rules.
    #[test]
    fn answer_vs_dispatch_phrase_mutex() {
        use agent_loop::r#loop::config::load_system_prompt;

        let answer = load_system_prompt("prompts/deprecated/orchestrator-multiagent/product-answer-base.md")
            .expect("product-answer-base");
        let chat = load_system_prompt("prompts/orchestrators/chat-base.md").expect("chat-base");
        let orch = load_system_prompt("prompts/deprecated/orchestrator-multiagent/orchestrator-base.md")
            .expect("orchestrator-base");

        for forbidden in [
            "不写给用户看的最终长文",
            "不写给用户看的长文",
            "你不自己查文档、不自己上网",
            "只分配不行动",
        ] {
            assert!(
                !answer.contains(forbidden),
                "product-answer-base must not contain Dispatch phrase {forbidden:?}"
            );
            assert!(
                !chat.contains(forbidden),
                "chat-base must not contain Dispatch phrase {forbidden:?}"
            );
        }
        assert!(
            orch.contains("不写给用户看的最终长文")
                || orch.contains("不写给用户看的长文")
                || orch.contains("写用户可见终答"),
            "orchestrator-base must retain coordinator no-final-answer rule"
        );
        assert!(
            orch.contains("finish_answer") || orch.contains("delegate_chat"),
            "orchestrator-base must name exit tool"
        );
    }

    /// G-05 / G-06: utility tools resolve from registry; retrieval/delegate never.
    #[test]
    fn answer_mode_tools_for_retrieve_exposes_utility_forbids_retrieval() {
        use agent_tools::capability::CapabilityRegistry;

        let assembled = crate::assemble_mode(CapabilitySet::default()).unwrap();
        let mut answer_config = assembled.config;
        answer_config.tool_pool = crate::mode_assemble::utility_tool_pool();
        let reg = CapabilityRegistry::standard_cached();
        let tools = answer_config.tools_for_retrieve(reg);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"user_context"),
            "utility pool must resolve user_context: {names:?}"
        );
        assert!(
            names.contains(&"calculator"),
            "utility pool must resolve calculator: {names:?}"
        );
        assert!(
            names.contains(&"weather_query"),
            "utility pool must resolve weather_query: {names:?}"
        );
        for ban in [
            "dense_retrieval",
            "lexical_search",
            "graph_search",
            "web_search",
            "delegate_rag",
            "delegate_search",
            "delegate_chat",
            "finish_answer",
        ] {
            assert!(
                !names.iter().any(|n| *n == ban),
                "Answer retrieve tools must not include {ban}: {names:?}"
            );
        }
    }
}
