//! V2 ReAct orchestrator brain (AGENT_ORCHESTRATOR_V2).
//!
//! Step-wise LLM dispatch loop: the model calls `delegate_rag` /
//! `delegate_search` / `finish_answer` / `evidence_fetch`
//! (host-intercepted, never registered on the global catalog) and observes each
//! result before choosing the next action. No rule-based planning or
//! query-rewriting code — de-referencing and channel-appropriate briefs are the
//! model's reasoning, guided by `prompts/orchestrators/orchestrator-base.md`
//! (design §3.2).
//!
//! Code owns only: materialization, finish-gates, loop guards, the evidence
//! store, and marker finalization.

use std::collections::{HashMap, HashSet};

use agent_loop::events::AgentEventSink;
use agent_loop::runtime::AgentRequest;
use avrag_llm::{ChatMessage, LlmProvider};
use common::AppError;

use super::chat_exit::{direct_handoff, synthesize_handoff};
use super::host::{dispatch_channel, OrchestratedTurn, OrchestratorExecutor};
use super::invariant::missing_dispatches;
use super::materialize::materialize_channels;
use super::store::{EvidenceKind, EvidenceStore};
use super::worker_session::{SessionError, WorkerSession};
use super::types::{
    Channel, ChannelNote, ChatExitMode, ChatHandoff, DispatchRecord, PackStatus, TaskBrief,
};
use super::workers::{
    attach_store_retrieval_tool_results, finalize_answer_evidence, tool_failures,
    worker_observability_from_run, WorkerBriefObservability,
};
use crate::capabilities::CapabilitySet;

/// Feature flag: `AGENT_ORCHESTRATOR_V2=1` (or true/yes/on). Falls back to the
/// structural first-wave host when off or when no orchestrator llm is configured.
pub fn orchestrator_v2_enabled() -> bool {
    match std::env::var("AGENT_ORCHESTRATOR_V2") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        }
        Err(_) => false,
    }
}

const DEFAULT_MAX_ROUNDS: u8 = 6;
const MAX_REDISPATCH_PER_CHANNEL: usize = 2;
const MAX_NUDGE_ROUNDS: u8 = 2;
const EVIDENCE_FETCH_MAX_EIDS: usize = 4;
const EVIDENCE_FETCH_MAX_CHARS: usize = 2000;

struct LoopConfig {
    max_rounds: u8,
    temperature: f32,
    base_prompt: String,
}

fn orchestrator_loop_config() -> LoopConfig {
    let (max_rounds, temperature) = agent_loop::r#loop::config::load_mode_config("orchestrator")
        .map(|m| {
            (
                m.budget.max_iterations.max(1),
                m.temperature.unwrap_or(0.4) as f32,
            )
        })
        .unwrap_or((DEFAULT_MAX_ROUNDS, 0.4));
    let base_prompt = agent_loop::r#loop::config::load_system_prompt(
        "prompts/orchestrators/orchestrator-base.md",
    )
    .unwrap_or_else(|_| "你是 Context OS 的协调者：读懂问题、派活给检索同事、再移交答案撰写。".into());
    LoopConfig {
        max_rounds,
        temperature,
        base_prompt,
    }
}

fn tool_spec(name: &str, description: &str, input_schema: serde_json::Value) -> contracts::ToolSpec {
    contracts::ToolSpec {
        name: name.to_string(),
        version: "1".to_string(),
        description: description.to_string(),
        input_schema,
        output_schema: serde_json::json!({}),
    }
}

/// Orchestrator-facing dispatch notes (`## 给任务分配者`) of a channel's
/// capability manual — what the coordinator must know to write briefs
/// (2026-07-20 prompt design §3.2 / §5-A). Falls back to the full manual
/// (with a warning) when the section heading is missing.
fn channel_dispatch_manual(channel: Channel) -> Option<String> {
    let path = match channel {
        // U1: the orchestrator-facing section lives in its own dispatch file —
        // workers receive the capability manual without it.
        Channel::Rag => "prompts/orchestrators/capability-rag.dispatch.md",
        Channel::Search => "prompts/orchestrators/capability-search.dispatch.md",
    };
    let manual = agent_loop::r#loop::config::load_system_prompt(path)
        .map_err(|e| {
            tracing::warn!(path, error = %e, "capability manual load failed; dispatch notes skipped");
            e
        })
        .ok()?;
    dispatch_note_from_manual(path, &manual)
}

/// C8b: extract the `## 给任务分配者` section from a capability manual, or
/// fail loud — a manual without the section is a broken asset, so the channel
/// gets NOTHING injected (no silent full-manual dump into the coordinator
/// prompt) and the misconfiguration is logged with the manual path and the
/// missing heading. Split out so the behavior is unit-testable without
/// touching the prompt files.
fn dispatch_note_from_manual(path: &str, manual: &str) -> Option<String> {
    extract_dispatch_section(manual).or_else(|| {
        tracing::warn!(
            path,
            heading = "## 给任务分配者",
            "capability manual missing dispatch section; channel gets no manual injected"
        );
        None
    })
}

/// Slice the `## 给任务分配者` section out of a capability manual: from that
/// heading to the next `## ` heading (or EOF). `None` when absent — or when
/// the heading has no body, which is useless to the coordinator.
fn extract_dispatch_section(manual: &str) -> Option<String> {
    const HEADING: &str = "## 给任务分配者";
    let start = manual.find(HEADING)?;
    let section = &manual[start..];
    let end = section[HEADING.len()..]
        .find("\n## ")
        .map(|i| HEADING.len() + i)
        .unwrap_or(section.len());
    let section = section[..end].trim();
    if section[HEADING.len()..].trim().is_empty() {
        None
    } else {
        Some(section.to_string())
    }
}

/// True when the store holds at least one CITABLE entry (DocChunk / WebPage;
/// DocProfile orientation entries are never citable and do not count).
/// Drives the C1 finish_answer mode=direct guard.
fn store_has_citable_evidence(store: &EvidenceStore) -> bool {
    store
        .entries()
        .iter()
        .any(|e| matches!(e.kind, EvidenceKind::DocChunk | EvidenceKind::WebPage))
}

/// Tool surface for the orchestrator: only materialized channels are offered
/// (§7.1 — the model cannot summon channels the product did not select).
/// Memory tools (PG conversation history + long-term user profile) are offered
/// when chat persistence is wired — the orchestrator is the only agent in the
/// paradigm that resolves user intent, so anaphora/profile lookup lives here
/// (host-intercepted, same as `evidence_fetch`).
fn orchestrator_tools(channels: &[Channel], has_memory: bool) -> Vec<contracts::ToolSpec> {
    let goal_schema = |desc: &str| {
        serde_json::json!({
            "type": "object",
            "properties": {
                "goal": {"type": "string", "description": desc},
                "focus_terms": {"type": "array", "items": {"type": "string"}},
                "max_items": {"type": "integer"}
            },
            "required": ["goal"]
        })
    };
    let mut tools = Vec::new();
    for ch in channels {
        match ch {
            Channel::Rag => tools.push(tool_spec(
                "delegate_rag",
                "向通道 worker 投递任务/追问（同一轮内同通道只有一个 worker，再次投递=带记忆的追问，它会看到此前的检索与交接）。goal 必须自包含（去语境化）：说明文档身份/结构与要抽取的内容。worker 返回状态、新证据编号与摘要。",
                goal_schema("自包含子任务目标：文档身份 + 要检索/抽取什么"),
            )),
            Channel::Search => tools.push(tool_spec(
                "delegate_search",
                "向通道 worker 投递任务/追问（同一轮内同通道只有一个 worker，再次投递=带记忆的追问）。goal 必须自包含：可独立成立的公网检索主题（不依赖工作区上下文；默认中英双语）。worker 返回状态、新证据编号与摘要。",
                goal_schema("自包含公网检索主题（中英双语）"),
            )),
        }
    }
    tools.push(tool_spec(
        "evidence_fetch",
        "按证据编号（E1、E2…）读取证据全文。清单已在观察中给出；只取需要的编号。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "eids": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["eids"]
        }),
    ));
    tools.push(tool_spec(
        "finish_answer",
        "结束派活并进入答案撰写阶段（唯一用户出口）。所有已开启的检索通道必须至少派发过一次才能调用（否则被运行时拦截）。mode=synthesize 时携带证据清单与写作说明；mode=direct 时直接回答。注意：mode=direct 仅在证据库为空（纯聊天）时生效；证据库已有可引用证据时 direct 会被运行时改判为 synthesize。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {"type": "string", "enum": ["synthesize", "direct"]},
                "instruction": {"type": "string", "description": "给答案撰写者的写作指令：必须显式写明理解口径（问题的多种读法中你选择了哪种，一句话）+ 证据组织方式 + 对比维度"}
            },
            "required": ["mode"]
        }),
    ));
    if has_memory {
        tools.push(tool_spec(
            "conversation_history_load",
            "调取更早的对话历史（PG 存储，近序+全文混合检索）。当前 query 含代词/指示词/省略（\"它\"、\"这篇\"、\"那个\"）且已注入的近期历史不足以消解时调用。",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "检索关键词（从当前 query 提取的实体词）；留空返回最近历史"},
                    "scope": {"type": "string", "enum": ["workspace", "session"], "default": "workspace"},
                    "limit": {"type": "integer", "default": 20}
                }
            }),
        ));
        tools.push(tool_spec(
            "user_profile_load",
            "调取用户长期画像（专业领域、偏好风格、常见问题）。写 brief / instruction 前若对用户背景不确定可调用。",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ));
    }
    tools
}

fn chat_message(role: &str, content: impl Into<String>) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: content.into(),
        multimodal_content: None,
        name: None,
        tool_call_id: None,
        tool_calls: None,
        reasoning_content: None,
    }
}

/// Refresh-per-round system message: base doctrine + dispatch-view capability
/// notes + live turn state.
fn render_system_message(
    base_prompt: &str,
    dispatch_manuals: &[String],
    channels: &[Channel],
    store: &EvidenceStore,
    records: &[DispatchRecord],
    round: u8,
    max_rounds: u8,
    prefs: Option<&agent_loop::runtime::AgentUserPreferences>,
) -> ChatMessage {
    let mut s = String::new();
    s.push_str(base_prompt);
    for manual in dispatch_manuals {
        s.push_str("\n\n");
        s.push_str(manual);
    }
    s.push_str("\n\n## 本轮状态（运行时注入，每轮刷新）\n");
    if let Some(profile) = prefs.and_then(render_user_profile) {
        s.push_str(&profile);
    }
    s.push_str(&format!(
        "- 已开启的检索通道（各至少派发一次后才能 finish_answer）：{}\n",
        channels
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !store.source_docs().is_empty() {
        s.push_str("- 源文档：");
        s.push_str(
            &store
                .source_docs()
                .iter()
                .map(|d| match &d.genre {
                    Some(g) => format!("《{}》(genre: {})", d.file_name, g),
                    None => format!("《{}》", d.file_name),
                })
                .collect::<Vec<_>>()
                .join("、"),
        );
        s.push('\n');
    }
    s.push_str(&format!("- 预算：round {}/{max_rounds}\n", round + 1));
    if !records.is_empty() {
        s.push_str("- 已派发：");
        s.push_str(
            &records
                .iter()
                .map(|r| {
                    format!(
                        "{}({:?}, {}条)",
                        r.channel.as_str(),
                        r.status,
                        r.item_count
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        s.push('\n');
    }
    s.push_str(&format!(
        "- 证据库：共 {} 条（rag {} / web {}）\n",
        store.entries().len(),
        store.count_channel(Channel::Rag),
        store.count_channel(Channel::Search)
    ));
    chat_message("system", s)
}

/// Compact user-profile line for the orchestrator system message — only the
/// small intent-relevant fields (never the raw custom_preferences blob, which
/// would burn tokens every round).
fn render_user_profile(prefs: &agent_loop::runtime::AgentUserPreferences) -> Option<String> {
    let mut parts = Vec::new();
    if !prefs.expertise_domains.is_empty() {
        parts.push(format!("专业领域: {}", prefs.expertise_domains.join("、")));
    }
    if let Some(style) = prefs
        .preferred_answer_style
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        parts.push(format!("偏好风格: {style}"));
    }
    if !prefs.frequently_asked_topics.is_empty() {
        parts.push(format!("近期关注: {}", prefs.frequently_asked_topics.join("、")));
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("- 用户画像：{}\n", parts.join("；")))
    }
}

fn parse_brief(channel: Channel, args: &serde_json::Value) -> Result<TaskBrief, String> {
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .ok_or_else(|| format!("delegate_{}: goal 不能为空", channel.as_str()))?;
    let mut brief = TaskBrief::new(goal);
    if let Some(terms) = args.get("focus_terms").and_then(|v| v.as_array()) {
        brief.focus_terms = terms
            .iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect();
    }
    brief.max_items = args
        .get("max_items")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);
    Ok(brief)
}

fn tool_result_msg(call_id: &str, name: &str, ok: bool, data: serde_json::Value) -> ChatMessage {
    let result = contracts::ToolResult {
        tool: name.to_string(),
        version: "1".into(),
        status: if ok {
            contracts::ToolStatus::Ok
        } else {
            contracts::ToolStatus::Error
        },
        data: Some(data),
        trace: None,
    };
    write_core::build_tool_message(call_id, name, &result)
}

fn guard_error(call_id: &str, name: &str, msg: impl Into<String>) -> ChatMessage {
    tool_result_msg(call_id, name, false, serde_json::json!({"error": msg.into()}))
}

/// True when search has already run ≥2 times without usable evidence.
/// Blocks further `delegate_search` so the brain does not burn redispatches on 空转
/// (first empty may still re-dispatch with a different goal once).
fn search_channel_exhausted(records: &[DispatchRecord]) -> bool {
    let search: Vec<&DispatchRecord> = records
        .iter()
        .filter(|r| r.channel == Channel::Search)
        .collect();
    if search.iter().any(|r| r.status == PackStatus::Ok && r.item_count > 0) {
        return false;
    }
    search
        .iter()
        .filter(|r| matches!(r.status, PackStatus::Empty | PackStatus::Error))
        .count()
        >= 2
}

/// V2 entry: step-wise LLM orchestrated turn.
#[allow(clippy::too_many_arguments)]
pub async fn run_llm_orchestrated_turn(
    caps: CapabilitySet,
    base_request: &AgentRequest,
    executor: &dyn OrchestratorExecutor,
    sink: &dyn AgentEventSink,
    docscope: Option<&common::DocScopeMetadata>,
    llm: &dyn LlmProvider,
    memory: Option<&std::sync::Arc<dyn app_core::ChatPersistencePort>>,
) -> Result<OrchestratedTurn, AppError> {
    let label = caps.agent_type_label().to_string();
    let channels = materialize_channels(caps);
    let query = base_request.query.clone();

    // Pure chat: no orchestrator loop, no workers (unchanged from V1).
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

    let config = orchestrator_loop_config();
    let tools = orchestrator_tools(&channels, memory.is_some());
    // Dispatch-view capability notes for the coordinator, loaded once per turn
    // (2026-07-20 prompt design §5-A); injected into every round's system.
    let dispatch_manuals: Vec<String> = channels
        .iter()
        .filter_map(|ch| channel_dispatch_manual(*ch))
        .collect();
    // Turn-start fact (2026-07-23): the brain's round-0 LLM call can take tens
    // of seconds — the client gets an immediate step instead of dead air.
    agent_loop::progress::emit_work_fact(sink, agent_loop::progress::WorkFact::understand(&query))
        .await;
    let mut store = EvidenceStore::from_docscope(docscope);
    let mut records: Vec<DispatchRecord> = Vec::new();
    let mut channel_notes = Vec::new();
    let mut worker_observability: Vec<WorkerBriefObservability> = Vec::new();
    // W1: per-channel persistent worker sessions (one turn lifetime).
    let mut sessions: std::collections::HashMap<Channel, WorkerSession> =
        std::collections::HashMap::new();
    let mut dispatch_counts: HashMap<Channel, usize> = HashMap::new();
    let mut seen_goals: HashSet<(Channel, String)> = HashSet::new();

    let mut messages: Vec<ChatMessage> = vec![chat_message("system", String::new())];
    for m in &base_request.messages {
        if m.role == "user" || m.role == "assistant" {
            messages.push(chat_message(&m.role, m.content.clone()));
        }
    }
    messages.push(chat_message("user", query.clone()));

    let mut nudge_rounds = 0u8;

    for round in 0..config.max_rounds {
        // Per-round thinking fact (2026-07-23): the brain's LLM call below is
        // the last silent stretch between retrieval facts and the answer.
        agent_loop::progress::emit_work_fact(
            sink,
            agent_loop::progress::WorkFact::thinking(None),
        )
        .await;
        messages[0] = render_system_message(
            &config.base_prompt,
            &dispatch_manuals,
            &channels,
            &store,
            &records,
            round,
            config.max_rounds,
            base_request.user_preferences.as_ref(),
        );
        let resp = llm
            .complete_with_tools(&messages, &tools, Some(config.temperature))
            .await
            .map_err(|e| AppError::internal(format!("orchestrator llm call failed: {e}")))?;
        let calls = resp.tool_calls.clone().unwrap_or_default();
        tracing::info!(
            round,
            tool_calls = ?calls.iter().map(|c| c.tool.as_str()).collect::<Vec<_>>(),
            "orchestrator round"
        );

        if calls.is_empty() {
            // Reasoning-only round: keep the thought, nudge to act.
            nudge_rounds += 1;
            if !resp.content.trim().is_empty() {
                messages.push(chat_message("assistant", resp.content.clone()));
            }
            if nudge_rounds >= MAX_NUDGE_ROUNDS {
                break;
            }
            messages.push(chat_message(
                "user",
                "继续：请调用一个工具（delegate_* / evidence_fetch / finish_answer）。",
            ));
            continue;
        }
        nudge_rounds = 0;

        let call_ids: Vec<String> = (0..calls.len())
            .map(|i| format!("call_{round}_{i}"))
            .collect();
        messages.push(write_core::build_assistant_message_with_tool_calls(
            &calls,
            &call_ids,
            &resp.content,
            resp.reasoning_content.clone(),
        ));

        // Execute delegates in this response concurrently (parallel wave);
        // chat / fetch / errors are handled sequentially in order.
        let mut tool_msgs: Vec<(String, ChatMessage)> = Vec::new();
        let mut chat_call: Option<(&contracts::ToolCall, String)> = None;

        // 1) Delegate calls: validate guards first.
        let mut wave: Vec<(String, Channel, TaskBrief)> = Vec::new();
        for (call, call_id) in calls.iter().zip(call_ids.iter()) {
            let channel = match call.tool.as_str() {
                "delegate_rag" => Some(Channel::Rag),
                "delegate_search" => Some(Channel::Search),
                _ => None,
            };
            let Some(channel) = channel else { continue };
            if !channels.contains(&channel) {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(call_id, &call.tool, "该通道未被产品 capabilities 选择，不能派发"),
                ));
                continue;
            }
            if dispatch_counts.get(&channel).copied().unwrap_or(0) >= MAX_REDISPATCH_PER_CHANNEL {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(
                        call_id,
                        &call.tool,
                        format!(
                            "通道 {} 已派发 {} 次（上限）。用 evidence_fetch 深读已有证据，或 finish_answer",
                            channel.as_str(),
                            MAX_REDISPATCH_PER_CHANNEL
                        ),
                    ),
                ));
                continue;
            }
            // Search 空转早收敛（编排层）：已有 Empty/Error 且从未 Ok 时，禁止再派 search。
            if channel == Channel::Search && search_channel_exhausted(&records) {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(
                        call_id,
                        &call.tool,
                        "search 已连续空结果/失败且无可用网页证据；不要再 delegate_search，请用已有证据 finish_answer 并说明网页侧未命中",
                    ),
                ));
                continue;
            }
            let brief = match parse_brief(channel, &call.args) {
                Ok(b) => b,
                Err(e) => {
                    tool_msgs.push((call_id.clone(), guard_error(call_id, &call.tool, e)));
                    continue;
                }
            };
            let key = (channel, brief.goal.trim().to_lowercase());
            if seen_goals.contains(&key) {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(
                        call_id,
                        &call.tool,
                        "相同 goal 已派发过，结果不会变化。换一个角度/关键词，或用 evidence_fetch",
                    ),
                ));
                continue;
            }
            seen_goals.insert(key);
            *dispatch_counts.entry(channel).or_insert(0) += 1;
            wave.push((call_id.clone(), channel, brief));
        }

        // 2) Run the wave: per-channel worker sessions (W1) — cross-channel
        // parallel, same-channel sequential through the SAME session.
        if !wave.is_empty() {
            for (_, channel, brief) in &wave {
                agent_loop::progress::emit_work_fact(
                    sink,
                    super::host::delegate_fact(*channel, brief),
                )
                .await;
            }
            // Group by channel, preserving first-seen channel order and the
            // brief order within each channel.
            let mut by_channel: Vec<(Channel, Vec<(String, TaskBrief)>)> = Vec::new();
            for (call_id, channel, brief) in wave {
                match by_channel.iter_mut().find(|(c, _)| *c == channel) {
                    Some((_, briefs)) => briefs.push((call_id, brief)),
                    None => by_channel.push((channel, vec![(call_id, brief)])),
                }
            }
            let runs = futures::future::join_all(by_channel.into_iter().map(
                |(channel, briefs)| {
                    // Failure isolation (W1): a poisoned session is dropped
                    // and replaced before this wave.
                    let mut session = sessions
                        .remove(&channel)
                        .filter(|s| !s.failed)
                        .unwrap_or_else(|| WorkerSession::new(channel));
                    async move {
                        let mut outs = Vec::new();
                        for (call_id, brief) in briefs {
                            let r = session.run_brief(&brief, base_request, executor).await;
                            outs.push((call_id, r));
                        }
                        (channel, session, outs)
                    }
                },
            ))
            .await;
            for (channel, session, outs) in runs {
                sessions.insert(channel, session);
                for (call_id, result) in outs {
                    match result {
                        Ok(Ok(outcome)) => {
                            let run = &outcome.run;
                            let inserted = store
                                .insert_from_tool_results(channel, &outcome.tool_results_delta);
                            let failures = tool_failures(&outcome.tool_results_delta);
                            let (status, error) = if inserted > 0 {
                                (PackStatus::Ok, None)
                            } else if !failures.is_empty() {
                                // Retrieval itself failed — NOT 未命中.
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
                            records.push(DispatchRecord {
                                channel,
                                dispatch_id: uuid::Uuid::new_v4().to_string(),
                                status,
                                item_count: inserted,
                                error: error.clone(),
                            });
                            // S5: soft fact verification (default OFF) — after the
                            // compile, before the note/store finalize.
                            let mut handoff = outcome.handoff.clone();
                            if let Some(h) = handoff.as_mut() {
                                super::fact_verify::verify_handoff_facts(
                                    h,
                                    &outcome.tool_results_delta,
                                )
                                .await;
                            }
                            worker_observability.push(WorkerBriefObservability {
                                channel,
                                seq: outcome.seq,
                                handoff_degraded: handoff
                                    .as_ref()
                                    .map(|h| h.handoff_degraded)
                                    .unwrap_or(false),
                                run: worker_observability_from_run(channel, run),
                            });
                            // K2/W1: hydrate the brief's SELECTED log (offset
                            // applied by the session) into the store's ★ tier.
                            store.mark_selected_for_brief(channel, &outcome.hydrated, Some(outcome.seq));
                            let note = ChannelNote::with_handoff(
                                channel,
                                status,
                                inserted,
                                handoff.clone(),
                                error.clone(),
                            );
                            channel_notes.push(note);
                            let new_listings: Vec<serde_json::Value> = store
                                .listings()
                                .into_iter()
                                .rev()
                                .take(inserted)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .map(|l| {
                                    serde_json::json!({
                                        "eid": l.eid, "label": l.label, "preview": l.preview,
                                    })
                                })
                                .collect();
                            let handoff_json = handoff
                                .as_ref()
                                .and_then(|h| serde_json::to_value(h).ok())
                                .unwrap_or(serde_json::Value::Null);
                            tool_msgs.push((
                                call_id.clone(),
                                tool_result_msg(
                                    &call_id,
                                    &format!("delegate_{}", channel.as_str()),
                                    status != PackStatus::Error,
                                    serde_json::json!({
                                        "channel": channel.as_str(),
                                        "status": format!("{status:?}").to_lowercase(),
                                        "brief_seq": outcome.seq,
                                        "new_evidence_count": inserted,
                                        "new_evidence": new_listings,
                                        "worker_handoff": handoff_json,
                                        "worker_digest": handoff.as_ref().map(|h| &h.summary),
                                        "tool_errors": error,
                                        "total_evidence": {
                                            "rag": store.count_channel(Channel::Rag),
                                            "search": store.count_channel(Channel::Search),
                                        }
                                    }),
                                ),
                            ));
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(channel = channel.as_str(), error = %e, "orchestrator dispatch failed");
                            records.push(DispatchRecord {
                                channel,
                                dispatch_id: uuid::Uuid::new_v4().to_string(),
                                status: PackStatus::Error,
                                item_count: 0,
                                error: Some(e.to_string()),
                            });
                            channel_notes.push(ChannelNote::with_handoff(
                                channel,
                                PackStatus::Error,
                                0,
                                None,
                                Some(e.to_string()),
                            ));
                            tool_msgs.push((
                                call_id.clone(),
                                tool_result_msg(
                                    &call_id,
                                    &format!("delegate_{}", channel.as_str()),
                                    false,
                                    serde_json::json!({
                                        "channel": channel.as_str(),
                                        "status": "error",
                                        "error": e.to_string(),
                                    }),
                                ),
                            ));
                        }
                        Err(SessionError::BudgetExhausted) => {
                            // W2: channel cap spent — the seal is visible to
                            // the brain as an error it can react to.
                            let msg = "channel budget exhausted（通道预算耗尽）：不要再向该通道投递 brief，用已有证据 finish_answer".to_string();
                            tracing::info!(channel = channel.as_str(), "{msg}");
                            records.push(DispatchRecord {
                                channel,
                                dispatch_id: uuid::Uuid::new_v4().to_string(),
                                status: PackStatus::Error,
                                item_count: 0,
                                error: Some(msg.clone()),
                            });
                            tool_msgs.push((
                                call_id.clone(),
                                tool_result_msg(
                                    &call_id,
                                    &format!("delegate_{}", channel.as_str()),
                                    false,
                                    serde_json::json!({
                                        "channel": channel.as_str(),
                                        "status": "error",
                                        "error": msg,
                                    }),
                                ),
                            ));
                        }
                    }
                }
            }
        }

        // 3) Non-delegate calls in order.
        for (call, call_id) in calls.iter().zip(call_ids.iter()) {
            match call.tool.as_str() {
                "delegate_rag" | "delegate_search" => {}
                "evidence_fetch" => {
                    let eids: Vec<String> = call
                        .args
                        .get("eids")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| e.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    let mut items = Vec::new();
                    for eid in eids.iter().take(EVIDENCE_FETCH_MAX_EIDS) {
                        match store.get(eid) {
                            Some(entry) => items.push(serde_json::json!({
                                "eid": entry.eid,
                                "label": entry.title.clone().or(entry.doc_name.clone()),
                                "full_text": entry.full_text.chars().take(EVIDENCE_FETCH_MAX_CHARS).collect::<String>(),
                            })),
                            None => items.push(serde_json::json!({
                                "eid": eid, "error": "证据库中不存在该编号",
                            })),
                        }
                    }
                    tool_msgs.push((
                        call_id.clone(),
                        tool_result_msg(
                            call_id,
                            "evidence_fetch",
                            true,
                            serde_json::json!({"items": items}),
                        ),
                    ));
                }
                "finish_answer" => {
                    chat_call = Some((call, call_id.clone()));
                }
                "conversation_history_load" => {
                    let session_uuid = base_request
                        .session_id
                        .as_deref()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());
                    let result = match (memory, session_uuid) {
                        (Some(port), Some(sid)) => {
                            agent_tools::skills::memory_dispatch::conversation_history_load(
                                &call.args,
                                &base_request.auth,
                                sid,
                                port.as_ref(),
                            )
                            .await
                        }
                        _ => agent_tools::skills::memory_dispatch::memory_tool_error(
                            "conversation_history_load",
                            "无会话或存储上下文",
                        ),
                    };
                    tool_msgs.push((
                        call_id.clone(),
                        write_core::build_tool_message(call_id, "conversation_history_load", &result),
                    ));
                }
                "user_profile_load" => {
                    let result = match memory {
                        Some(port) => {
                            agent_tools::skills::memory_dispatch::user_profile_load(
                                &base_request.auth,
                                port.as_ref(),
                            )
                            .await
                        }
                        None => agent_tools::skills::memory_dispatch::memory_tool_error(
                            "user_profile_load",
                            "存储未配置",
                        ),
                    };
                    tool_msgs.push((
                        call_id.clone(),
                        write_core::build_tool_message(call_id, "user_profile_load", &result),
                    ));
                }
                other => {
                    tool_msgs.push((
                        call_id.clone(),
                        guard_error(call_id, other, format!("未知工具 {other}")),
                    ));
                }
            }
        }

        // 4) finish_answer / delegate_chat last (it may finish the turn).
        if let Some((call, call_id)) = chat_call {
            let missing = missing_dispatches(&channels, &records);
            if !missing.is_empty() {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(
                        &call_id,
                        call.tool.as_str(),
                        format!(
                            "finish-gate：通道 {} 尚未派发，先 delegate 这些通道",
                            missing
                                .iter()
                                .map(|c| c.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ),
                ));
                messages.extend(tool_msgs.into_iter().map(|(_, m)| m));
                continue;
            }
            let mode = call
                .args
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("synthesize");
            // Strict validation: unknown mode values are rejected instead of
            // silently falling back to synthesize.
            if mode != "synthesize" && mode != "direct" {
                tool_msgs.push((
                    call_id.clone(),
                    guard_error(
                        &call_id,
                        call.tool.as_str(),
                        format!("未知 mode={mode}；只支持 synthesize 或 direct"),
                    ),
                ));
                messages.extend(tool_msgs.into_iter().map(|(_, m)| m));
                continue;
            }
            let instruction = call
                .args
                .get("instruction")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            // C1: mode=direct is only valid for zero-evidence pure-chat turns.
            // When the store already holds CITABLE evidence (DocChunk/WebPage
            // — DocProfile orientation entries do not count), a direct handoff
            // would discard the whole evidence package and the Answer phase
            // truthfully reports "no materials" (2026-07-27 autopsy class).
            // Override to synthesize and trace the reason.
            let mode = if mode == "direct" && store_has_citable_evidence(&store) {
                tracing::warn!(
                    citable = true,
                    "finish_answer mode=direct overridden to synthesize: \
                     evidence store holds citable evidence"
                );
                "synthesize"
            } else {
                mode
            };
            let handoff: ChatHandoff = if mode == "direct" {
                direct_handoff(&query)
            } else {
                synthesize_handoff(
                    &query,
                    store.source_docs().to_vec(),
                    store.listings(),
                    store.targeted_entries(),
                    channel_notes.clone(),
                    &records,
                    instruction,
                )
            };
            agent_loop::progress::emit_work_fact(
                sink,
                agent_loop::progress::WorkFact::compose_answer(),
            )
            .await;
            let mut answer_result = executor.run_chat(&handoff, base_request, sink).await?;
            // En→cite finalize only for synthesize: a direct handoff carries no
            // evidence pack in the query, so markers must not be rewritten into
            // citations for passages the answer never grounded on (§5.3.1).
            // Still attach store→retrieval tool_results when workers filled the
            // store (eval bridge / observability); direct does not invent cites.
            if handoff.mode == ChatExitMode::Synthesize {
                finalize_answer_evidence(&mut answer_result, &store);
            } else {
                attach_store_retrieval_tool_results(&mut answer_result, &store);
            }
            return Ok(OrchestratedTurn {
                answer_result,
                store,
                records,
                handoff,
                agent_type_label: label,
                worker_observability,
            });
        }

        messages.extend(tool_msgs.into_iter().map(|(_, m)| m));
    }

    // Budget exhausted (or model kept reasoning without acting): deterministic
    // finish — run missing channels with the policy-free default brief, then
    // a forced synthesize chat (§7.2 holds by construction).
    tracing::warn!("orchestrator budget exhausted; forcing deterministic finish");
    for ch in missing_dispatches(&channels, &records) {
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
        records.push(outcome.record);
        channel_notes.push(outcome.note);
        if let Some(obs) = outcome.observability {
            worker_observability.push(obs);
        }
    }
    let handoff = synthesize_handoff(
        &query,
        store.source_docs().to_vec(),
        store.listings(),
        store.targeted_entries(),
        channel_notes,
        &records,
        Some("编排预算已用完：请基于已获证据直接合成；缺口如实说明。".into()),
    );
    agent_loop::progress::emit_work_fact(
        sink,
        agent_loop::progress::WorkFact::compose_answer(),
    )
    .await;
    let mut answer_result = executor.run_chat(&handoff, base_request, sink).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_loop::events::CollectingSink;
    use agent_loop::runtime::AgentRunResult;
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use uuid::Uuid;

    #[test]
    fn search_exhausted_after_two_empty_not_after_one() {
        let one = vec![DispatchRecord {
            channel: Channel::Search,
            dispatch_id: "d1".into(),
            status: PackStatus::Empty,
            item_count: 0,
            error: None,
        }];
        assert!(!search_channel_exhausted(&one));
        let two = vec![
            DispatchRecord {
                channel: Channel::Search,
                dispatch_id: "d1".into(),
                status: PackStatus::Empty,
                item_count: 0,
                error: None,
            },
            DispatchRecord {
                channel: Channel::Search,
                dispatch_id: "d2".into(),
                status: PackStatus::Error,
                item_count: 0,
                error: Some("timeout".into()),
            },
        ];
        assert!(search_channel_exhausted(&two));
        let ok_then = vec![
            DispatchRecord {
                channel: Channel::Search,
                dispatch_id: "d1".into(),
                status: PackStatus::Empty,
                item_count: 0,
                error: None,
            },
            DispatchRecord {
                channel: Channel::Search,
                dispatch_id: "d2".into(),
                status: PackStatus::Ok,
                item_count: 3,
                error: None,
            },
        ];
        assert!(!search_channel_exhausted(&ok_then));
    }

    #[test]
    fn dispatch_section_runs_from_heading_to_next_heading() {
        let manual = "## 能力：X\n\n执行协议正文\n\n## 给任务分配者\n\n- 能做什么：测试\n- 何时再派：必要时\n\n## 约束\n\n尾部\n";
        let section = extract_dispatch_section(manual).expect("section present");
        assert!(section.starts_with("## 给任务分配者"));
        assert!(section.contains("能做什么：测试"));
        assert!(!section.contains("执行协议正文"));
        assert!(!section.contains("尾部"));
    }

    #[test]
    fn dispatch_section_absent_or_empty_returns_none() {
        assert!(extract_dispatch_section("## 只有执行协议\n正文").is_none());
        assert!(extract_dispatch_section("## 给任务分配者\n\n## 约束\nx").is_none());
    }

    #[test]
    fn dispatch_manual_loads_from_real_capability_files() {
        // 派活小节是编排 system 的能力来源；文件缺节时该通道什么都拿不到
        //（C8b fail-loud），所以正式 prompts 必须带 `## 给任务分配者`（本测试锁死这一约定）。
        let rag = channel_dispatch_manual(Channel::Rag).expect("rag manual");
        assert!(rag.starts_with("## 给任务分配者"), "{rag}");
        assert!(rag.contains("coverage"), "{rag}");
        let search = channel_dispatch_manual(Channel::Search).expect("search manual");
        assert!(search.starts_with("## 给任务分配者"), "{search}");
    }

    #[test]
    fn manual_without_dispatch_heading_injects_nothing() {
        // C8b: broken manual (no 给任务分配者 section, or an empty one) →
        // fail loud: no injection at all (previously the FULL worker manual
        // was silently dumped into the coordinator prompt).
        let no_heading = "# capability-rag\n\n只做检索的工作说明，没有派活小节。";
        assert!(
            dispatch_note_from_manual("prompts/orchestrators/capability-rag.md", no_heading)
                .is_none()
        );
        let empty_heading = "## 给任务分配者\n\n## 其他\n\n正文";
        assert!(
            dispatch_note_from_manual("prompts/orchestrators/capability-rag.md", empty_heading)
                .is_none()
        );
        // Sanity: a proper manual still yields its section.
        let good = "## 给任务分配者\n\n- 能做什么：测试\n";
        let note = dispatch_note_from_manual("x.md", good).expect("section");
        assert!(note.contains("能做什么：测试"));
    }

    #[test]
    fn system_message_orders_base_then_manuals_then_state() {
        let msg = render_system_message(
            "BASE",
            &["## 给任务分配者\n\n- 能做什么：测试".to_string()],
            &[Channel::Rag],
            &EvidenceStore::default(),
            &[],
            0,
            6,
            None,
        );
        let base = msg.content.find("BASE").expect("base");
        let manual = msg.content.find("给任务分配者").expect("manual");
        let state = msg.content.find("本轮状态").expect("state");
        assert!(base < manual && manual < state, "{}", msg.content);
        assert!(msg.content.contains("已开启的检索通道"), "{}", msg.content);
    }

    struct ScriptedLlm {
        responses: Mutex<VecDeque<avrag_llm::LlmResponse>>,
        recorded: Mutex<Vec<Vec<ChatMessage>>>,
    }

    impl ScriptedLlm {
        fn new(responses: Vec<avrag_llm::LlmResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                recorded: Mutex::new(Vec::new()),
            }
        }

        fn recorded(&self) -> Vec<Vec<ChatMessage>> {
            self.recorded.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for ScriptedLlm {
        async fn complete(
            &self,
            _messages: &[ChatMessage],
            _temperature: Option<f32>,
        ) -> anyhow::Result<avrag_llm::LlmResponse> {
            anyhow::bail!("scripted llm: complete not used")
        }

        async fn complete_with_tools(
            &self,
            messages: &[ChatMessage],
            _tools: &[contracts::ToolSpec],
            _temperature: Option<f32>,
        ) -> anyhow::Result<avrag_llm::LlmResponse> {
            self.recorded.lock().unwrap().push(messages.to_vec());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("scripted llm exhausted"))
        }
    }

    fn mock_usage() -> avrag_llm::LlmUsage {
        avrag_llm::LlmUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            provider: String::new(),
            model: String::new(),
            cached_tokens: 0,
        }
    }

    fn tool_call_response(calls: Vec<(&str, serde_json::Value)>) -> avrag_llm::LlmResponse {
        avrag_llm::LlmResponse {
            content: String::new(),
            reasoning_content: None,
            usage: mock_usage(),
            model: "mock".into(),
            tool_calls: Some(
                calls
                    .into_iter()
                    .map(|(tool, args)| contracts::ToolCall {
                        tool: tool.into(),
                        version: "1".into(),
                        args,
                    })
                    .collect(),
            ),
        }
    }

    fn prose_response(text: &str) -> avrag_llm::LlmResponse {
        avrag_llm::LlmResponse {
            content: text.into(),
            reasoning_content: None,
            usage: mock_usage(),
            model: "mock".into(),
            tool_calls: None,
        }
    }

    fn delegate(channel: &str, goal: &str) -> (&'static str, serde_json::Value) {
        let tool = match channel {
            "rag" => "delegate_rag",
            _ => "delegate_search",
        };
        (tool, serde_json::json!({"goal": goal}))
    }

    fn chat_call() -> (&'static str, serde_json::Value) {
        (
            "finish_answer",
            serde_json::json!({"mode": "synthesize", "instruction": "对比分析"}),
        )
    }

    fn finish_answer_call() -> (&'static str, serde_json::Value) {
        (
            "finish_answer",
            serde_json::json!({"mode": "synthesize", "instruction": "对比分析"}),
        )
    }

    fn finish_answer_direct_call() -> (&'static str, serde_json::Value) {
        (
            "finish_answer",
            serde_json::json!({"mode": "direct"}),
        )
    }

    fn finish_answer_invalid_mode_call() -> (&'static str, serde_json::Value) {
        (
            "finish_answer",
            serde_json::json!({"mode": "unknown_mode"}),
        )
    }

    struct BrainMockExec;
    #[async_trait::async_trait]
    impl OrchestratorExecutor for BrainMockExec {
        async fn run_channel(
            &self,
            channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = format!("{} digest", channel.as_str());
            r.tool_results = vec![match channel {
                Channel::Rag => contracts::ToolResult {
                    tool: "dense_retrieval".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!([
                        {"chunk_id": "chunk-a", "doc_id": "d1", "text": "doc evidence", "score": 0.9}
                    ])),
                    trace: None,
                },
                Channel::Search => contracts::ToolResult {
                    tool: "web_search".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!({
                        "results": [{"url": "https://a.example", "title": "A", "snippet": "web evidence"}]
                    })),
                    trace: None,
                },
            }];
            Ok(r)
        }

        async fn run_chat(
            &self,
            handoff: &ChatHandoff,
            _base: &AgentRequest,
            _sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            // A real answer writer only emits E-markers when it saw evidence
            // (synthesize); direct answers cite nothing.
            r.answer = match handoff.mode {
                ChatExitMode::Synthesize => "文档证据 [[E1]]，网页佐证 [[E2]]。".into(),
                ChatExitMode::Direct => "直接回答，无引用。".into(),
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
            doc_scope: vec!["d1".into()],
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

    fn dual() -> CapabilitySet {
        CapabilitySet {
            rag: true,
            search: true,
        }
    }

    #[tokio::test]
    async fn sequential_dispatch_then_chat() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "定向文档并抽取结构")]),
            tool_call_response(vec![delegate("search", "数字化转型 立项报告 最佳实践 digital transformation best practices")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
        assert!(turn.answer_result.answer.contains("[[web:2]]"));
        assert_eq!(turn.answer_result.citations.len(), 2);
    }

    /// C1: worker filled the store with a citable doc chunk, yet the brain
    /// asks finish_answer(mode=direct) → runtime must override to synthesize
    /// so the evidence package survives (2026-07-27 autopsy empty-desk class).
    #[tokio::test]
    async fn direct_finish_overridden_when_store_has_citable_evidence() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "文档结构")]),
            tool_call_response(vec![delegate("search", "best practices")]),
            tool_call_response(vec![finish_answer_direct_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        // BrainMockExec answers with E-markers only for Synthesize handoffs —
        // a citation in the final answer proves the override fired.
        assert!(
            turn.answer_result.answer.contains("[[cite:chunk-a]]"),
            "{}",
            turn.answer_result.answer
        );
        assert_eq!(turn.handoff.mode, ChatExitMode::Synthesize);
    }

    /// Executor whose worker returns nothing (zero-evidence turn).
    struct EmptyStoreExec;
    #[async_trait::async_trait]
    impl OrchestratorExecutor for EmptyStoreExec {
        async fn run_channel(
            &self,
            _channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            Ok(AgentRunResult::default())
        }

        async fn run_chat(
            &self,
            handoff: &ChatHandoff,
            _base: &AgentRequest,
            _sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = match handoff.mode {
                ChatExitMode::Synthesize => "文档证据 [[E1]]。".into(),
                ChatExitMode::Direct => "直接回答，无引用。".into(),
            };
            Ok(r)
        }
    }

    #[tokio::test]
    async fn direct_finish_passes_when_store_is_empty() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "文档结构")]),
            tool_call_response(vec![delegate("search", "best practices")]),
            tool_call_response(vec![finish_answer_direct_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &EmptyStoreExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.handoff.mode, ChatExitMode::Direct);
        assert!(turn.answer_result.answer.contains("直接回答"));
    }

    /// Executor whose worker returns only a doc_profile (orientation-only,
    /// never citable) — direct must still pass through.
    struct DocProfileOnlyExec;
    #[async_trait::async_trait]
    impl OrchestratorExecutor for DocProfileOnlyExec {
        async fn run_channel(
            &self,
            _channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.tool_results = vec![contracts::ToolResult {
                tool: "doc_profile".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"doc_id": "d1", "doc_name": "thesis.txt", "summary": "orientation"}
                ])),
                trace: None,
            }];
            Ok(r)
        }

        async fn run_chat(
            &self,
            handoff: &ChatHandoff,
            _base: &AgentRequest,
            _sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = match handoff.mode {
                ChatExitMode::Synthesize => "文档证据 [[E1]]。".into(),
                ChatExitMode::Direct => "直接回答，无引用。".into(),
            };
            Ok(r)
        }
    }

    #[tokio::test]
    async fn direct_finish_passes_when_store_has_orientation_only() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "文档结构")]),
            tool_call_response(vec![delegate("search", "best practices")]),
            tool_call_response(vec![finish_answer_direct_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &DocProfileOnlyExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.handoff.mode, ChatExitMode::Direct);
        assert!(turn.answer_result.answer.contains("直接回答"));
    }

    #[tokio::test]
    async fn system_message_carries_dispatch_sections() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![
                delegate("rag", "文档结构"),
                delegate("search", "best practices"),
            ]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        let recorded = llm.recorded();
        let system = recorded[0]
            .iter()
            .find(|m| m.role == "system")
            .expect("system message");
        // rag + search 都物化 → 两份「给任务分配者」小节都在编排 system 里。
        assert_eq!(system.content.matches("## 给任务分配者").count(), 2, "{}", system.content);
        assert!(system.content.contains("公网 web search"), "{}", system.content);
        assert!(system.content.contains("已开启的检索通道"), "{}", system.content);
    }

    /// P3 probe: worker answers with a structured partial-coverage handoff.
    struct PartialCoverageExec;
    #[async_trait::async_trait]
    impl OrchestratorExecutor for PartialCoverageExec {
        async fn run_channel(
            &self,
            channel: Channel,
            _brief: &TaskBrief,
            _base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            let mut r = AgentRunResult::default();
            r.answer = serde_json::json!({
                "schema_version": "internal_worker_handoff_v1",
                "summary": format!("{} 摘要", channel.as_str()),
                "key_facts": [],
                "coverage": "partial",
                "gaps": ["投资估算章节未覆盖"]
            })
            .to_string();
            r.tool_results = vec![match channel {
                Channel::Rag => contracts::ToolResult {
                    tool: "dense_retrieval".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!([
                        {"chunk_id": "chunk-a", "doc_id": "d1", "text": "doc evidence", "score": 0.9}
                    ])),
                    trace: None,
                },
                Channel::Search => contracts::ToolResult {
                    tool: "web_search".into(),
                    version: "1".into(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::json!({
                        "results": [{"url": "https://a.example", "title": "A", "snippet": "web evidence"}]
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
            Ok(AgentRunResult::default())
        }
    }

    /// P3 探针（coverage=partial）：gaps 必须进入编排器的观察（它据此决定
    /// 再派），且换一个新 goal 的再派不被守卫拦截。
    #[tokio::test]
    async fn partial_coverage_is_observable_and_redispatch_allowed() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "第一轮取证")]),
            tool_call_response(vec![delegate("rag", "专查投资估算章节")]), // 新 goal → 必须放行
            tool_call_response(vec![delegate("search", "web 侧")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &PartialCoverageExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        let rag_records = turn
            .records
            .iter()
            .filter(|r| r.channel == Channel::Rag)
            .count();
        assert_eq!(rag_records, 2, "re-dispatch with a new goal must run");
        // 编排器在 tool 观察里看到了 coverage=partial 与 gaps。
        let recorded = llm.recorded();
        let obs: String = recorded[1]
            .iter()
            .filter(|m| m.role == "tool")
            .map(|m| m.content.clone())
            .collect();
        assert!(obs.contains("partial"), "{obs}");
        assert!(obs.contains("投资估算章节未覆盖"), "{obs}");
        // gaps 随 channel_notes 流入 Chat handoff。
        assert!(
            turn.handoff.channel_notes.iter().any(|n| {
                n.handoff
                    .as_ref()
                    .map(|h| h.gaps.iter().any(|g| g.contains("投资估算")))
                    .unwrap_or(false)
            }),
            "gaps must reach the chat handoff: {:?}",
            turn.handoff.channel_notes
        );
    }

    /// P3 探针（空 handoff）：首次 Empty 不封锁 rag 换 goal 再派
    /// （search 侧连续空结果的强制收敛已由 search_exhausted 系列测试覆盖）。
    #[tokio::test]
    async fn empty_first_result_does_not_block_rag_redispatch() {
        struct EmptyExec;
        #[async_trait::async_trait]
        impl OrchestratorExecutor for EmptyExec {
            async fn run_channel(
                &self,
                _channel: Channel,
                _brief: &TaskBrief,
                _base: &AgentRequest,
            ) -> Result<AgentRunResult, AppError> {
                Ok(AgentRunResult::default())
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

        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "第一次")]),
            tool_call_response(vec![delegate("rag", "换个角度再查")]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &EmptyExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        let rag: Vec<&DispatchRecord> = turn
            .records
            .iter()
            .filter(|r| r.channel == Channel::Rag)
            .collect();
        assert_eq!(rag.len(), 2, "empty first result must not block re-dispatch");
        assert!(rag.iter().all(|r| r.status == PackStatus::Empty));
    }

    #[tokio::test]
    async fn finish_gate_blocks_early_chat() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![chat_call()]), // gated: no dispatches yet
            tool_call_response(vec![delegate("rag", "先取证")]),
            tool_call_response(vec![delegate("search", "双语检索")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        // Chat was gated until both channels had records; final turn has both.
        assert_eq!(turn.records.len(), 2);
        assert_eq!(turn.answer_result.citations.len(), 2);
    }

    #[tokio::test]
    async fn batched_delegates_run_as_wave() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![
                delegate("rag", "文档结构"),
                delegate("search", "best practices"),
            ]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        assert_eq!(turn.store.count_channel(Channel::Rag), 1);
        assert_eq!(turn.store.count_channel(Channel::Search), 1);
    }

    #[tokio::test]
    async fn duplicate_goal_is_rejected() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "同一个目标")]),
            tool_call_response(vec![delegate("rag", "同一个目标")]), // dup → guard error
            tool_call_response(vec![delegate("search", "web 侧")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        let rag_records = turn
            .records
            .iter()
            .filter(|r| r.channel == Channel::Rag)
            .count();
        assert_eq!(rag_records, 1, "duplicate goal must not re-dispatch");
    }

    #[tokio::test]
    async fn budget_exhaustion_forces_deterministic_finish() {
        let llm = ScriptedLlm::new(vec![
            prose_response("让我想想……"),
            prose_response("还在思考……"),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        // Fallback dispatched both missing channels, then forced chat.
        assert_eq!(turn.records.len(), 2);
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
    }

    #[tokio::test]
    async fn evidence_fetch_is_served_from_store() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "取证")]),
            tool_call_response(vec![(
                "evidence_fetch",
                serde_json::json!({"eids": ["E1"]}),
            )]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        assert_eq!(turn.answer_result.citations.len(), 2);
    }

    #[tokio::test]
    async fn memory_tools_and_user_profile_reach_brain() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![
                ("user_profile_load", serde_json::json!({})),
                ("conversation_history_load", serde_json::json!({"query": "转型"})),
            ]),
            tool_call_response(vec![delegate("rag", "取证"), delegate("search", "web")]),
            tool_call_response(vec![chat_call()]),
        ]);
        let state = std::sync::Arc::new(tokio::sync::RwLock::new(app_core::MemoryState::default()));
        let workspace_id = Uuid::new_v4();
        {
            let now = common::now_rfc3339();
            state.write().await.workspaces.insert(
                workspace_id.to_string(),
                contracts::workspaces::Workspace {
                    id: workspace_id.to_string(),
                    owner_user_id: Uuid::nil().to_string(),
                    owner_id: Uuid::nil().to_string(),
                    name: "nb".into(),
                    title: "nb".into(),
                    description: String::new(),
                    document_count: 0,
                    status_summary: Default::default(),
                    shared: false,
                    created_at: now.clone(),
                    updated_at: now,
                },
            );
        }
        let persistence: std::sync::Arc<dyn app_core::ChatPersistencePort> =
            std::sync::Arc::new(app_core::MemoryChatPersistence::new(state));
        let mut req = base_req("差距在哪");
        req.auth = AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::User)
            .with_actor_id(contracts::auth_runtime::ActorId::new(Uuid::nil()));
        let session = persistence
            .create_session(&req.auth, workspace_id, None, "rag")
            .await
            .expect("seed session");
        req.session_id = Some(session.id.clone());
        req.user_preferences = Some(agent_loop::runtime::AgentUserPreferences {
            expertise_domains: vec!["企业IT基础设施".into()],
            preferred_answer_style: None,
            frequently_asked_topics: vec![],
            custom_preferences: serde_json::json!({}),
            structured_profile: serde_json::json!({}),
            inference_version: None,
        });
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &req,
            &BrainMockExec,
            &sink,
            None,
            &llm,
            Some(&persistence),
        )
        .await
        .unwrap();
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));

        let recorded = llm.recorded();
        // Every round's system message carries the compact profile line.
        let system = recorded[1]
            .iter()
            .find(|m| m.role == "system")
            .expect("system message");
        assert!(system.content.contains("用户画像"), "{}", system.content);
        assert!(
            system.content.contains("企业IT基础设施"),
            "{}",
            system.content
        );
        // Memory tool results (from the PG port, not the guard-error path)
        // were appended as tool messages for the next round.
        let tool_text: String = recorded[1]
            .iter()
            .filter(|m| m.role == "tool")
            .map(|m| m.content.clone())
            .collect();
        assert!(
            tool_text.contains("\"expertise_domains\":[]"),
            "{tool_text}"
        );
        assert!(tool_text.contains("message_count"), "{tool_text}");
        assert!(!tool_text.contains("存储未配置"), "{tool_text}");
    }

    #[tokio::test]
    async fn finish_answer_alias_works() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "取证")]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![finish_answer_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
    }

    #[tokio::test]
    async fn finish_answer_direct_overridden_to_synthesize_with_evidence() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "取证")]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![finish_answer_direct_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        // C1 (supersedes the old "direct skips evidence" contract): direct is
        // overridden to synthesize when the store holds citable evidence, so
        // the evidence package reaches the Answer phase and citations exist.
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
        assert!(!turn.answer_result.citations.is_empty());
        // Observability / eval: store→retrieval tool_results still attached when
        // workers filled the store (attach_store_retrieval_tool_results).
        assert!(
            turn.answer_result
                .tool_results
                .iter()
                .any(|t| t.tool == "dense_retrieval" || t.tool == "web_search"),
            "direct mode still surfaces store for eval when workers returned evidence: {:?}",
            turn.answer_result
                .tool_results
                .iter()
                .map(|t| &t.tool)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn finish_answer_invalid_mode_rejected() {
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "取证")]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![finish_answer_invalid_mode_call()]),
            // After rejection, LLM calls finish_answer correctly.
            tool_call_response(vec![finish_answer_call()]),
        ]);
        let sink = CollectingSink::new();
        let turn = run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &BrainMockExec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();
        assert_eq!(turn.records.len(), 2);
        assert!(turn.answer_result.answer.contains("[[cite:chunk-a]]"));
    }

    /// G-02: V2 path through real [`AgentServiceExecutor`] must assemble Option D Answer pack.
    #[tokio::test]
    async fn v2_answer_phase_uses_product_answer_pack_via_real_executor() {
        use crate::orchestrator::AgentServiceExecutor;
        use agent_loop::runtime::Agent;
        use crate::agents::AgentKind;

        /// Captures the last Chat-kind request (Answer phase); workers are Rag/Search.
        struct CaptureChatAgent {
            last_chat: std::sync::Arc<Mutex<Option<AgentRequest>>>,
        }
        #[async_trait::async_trait]
        impl Agent for CaptureChatAgent {
            async fn run(
                &self,
                request: AgentRequest,
                _sink: &dyn AgentEventSink,
            ) -> Result<AgentRunResult, AppError> {
                if request.kind == AgentKind::Chat {
                    *self.last_chat.lock().unwrap() = Some(request.clone());
                }
                // Workers need tool_results so store/finalize has something;
                // Answer phase can return empty prose.
                let mut r = AgentRunResult::default();
                match request.kind {
                    AgentKind::Rag => {
                        r.tool_results = vec![contracts::ToolResult {
                            tool: "dense_retrieval".into(),
                            version: "1".into(),
                            status: contracts::ToolStatus::Ok,
                            data: Some(serde_json::json!([{
                                "chunk_id": "chunk-a",
                                "doc_id": "d1",
                                "text": "doc evidence for pack",
                                "score": 0.9
                            }])),
                            trace: None,
                        }];
                    }
                    AgentKind::Search => {
                        r.tool_results = vec![contracts::ToolResult {
                            tool: "web_search".into(),
                            version: "1".into(),
                            status: contracts::ToolStatus::Ok,
                            data: Some(serde_json::json!({
                                "results": [{
                                    "url": "https://a.example",
                                    "title": "A",
                                    "snippet": "web evidence"
                                }]
                            })),
                            trace: None,
                        }];
                    }
                    AgentKind::Chat => {
                        r.answer = "ok".into();
                    }
                    _ => {}
                }
                Ok(r)
            }
        }

        let last_chat = std::sync::Arc::new(Mutex::new(None));
        let exec = AgentServiceExecutor::new(std::sync::Arc::new(
            crate::agents::service::UnifiedAgentService::new(Box::new(CaptureChatAgent {
                last_chat: last_chat.clone(),
            })),
        ));
        let llm = ScriptedLlm::new(vec![
            tool_call_response(vec![delegate("rag", "取证")]),
            tool_call_response(vec![delegate("search", "web")]),
            tool_call_response(vec![finish_answer_call()]),
        ]);
        let sink = CollectingSink::new();
        run_llm_orchestrated_turn(
            dual(),
            &base_req("差距在哪"),
            &exec,
            &sink,
            None,
            &llm,
            None,
        )
        .await
        .unwrap();

        let req = last_chat
            .lock()
            .unwrap()
            .clone()
            .expect("V2 must invoke Answer phase Chat agent");
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
            "V2 Answer pack must include product-answer-base: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("chat-base.md")),
            "P1-2: V2 Answer pack must not include full chat-base: {parts:?}"
        );
        assert!(
            !parts.iter().any(|p| p.contains("orchestrator-base")),
            "Answer phase must not load orchestrator-base: {parts:?}"
        );
        // Synthesize path: evidence in query (KD-16).
        assert!(
            req.query.contains("### Evidence") || req.query.contains("doc evidence"),
            "V2 synthesize Answer query should carry evidence context: {}",
            req.query.chars().take(240).collect::<String>()
        );

        // G-04 Dispatch side: brain system must not load answer-from blocks.
        let recorded = llm.recorded();
        assert!(!recorded.is_empty());
        let system = recorded[0]
            .iter()
            .find(|m| m.role == "system")
            .expect("brain records a system message");
        assert!(
            !system.content.contains("answer-from-workspace")
                && !system.content.contains("### Evidence (complete set"),
            "Dispatch system must not embed Answer evidence pack: {}",
            system.content.chars().take(200).collect::<String>()
        );
    }
}
