//! W1 (2026-07-28, channel-persistent worker design §3.1/§3.2): one worker
//! instance per channel per turn. Every brief for a channel is delivered to
//! the SAME `WorkerSession`, which carries forward the prior briefs' context
//! (compacted), the retrieval-log alias cursor, and cumulative budget —
//! "重派" becomes a follow-up question to a worker that remembers.
//!
//! Session state per brief ([`BriefRecord`]): goal, handoff summary,
//! coverage, SELECTED aliases, final message text, retrieval/tool counts,
//! iterations used. Resume context for brief N>1 is injected as user
//! messages built by [`resume_context_messages`]: older briefs ride as one
//! synthetic compaction entry each (summary + SELECTED aliases); the most
//! recent brief carries its full handoff text — the worker never cold-starts
//! and twins cannot exist on a channel.
//!
//! Budget semantics (W2): per-brief budget comes from the mode yaml (the
//! loop's own `max_iterations`); the channel total cap is
//! [`CHANNEL_ITERATION_CAP`] iterations per turn. A brief runs with
//! `min(per_brief_budget, cap_remaining)` — when the cap clamps a brief, the
//! loop's existing C5 budget-exhausted turn forces the handoff and the
//! session is SEALED (further briefs rejected with
//! [`SessionError::BudgetExhausted`], visible to the brain in the dispatch
//! error / debug map).
//!
//! Failure isolation: a hard executor error marks the session `failed`; the
//! dispatch layer drops it and creates a fresh one on the next brief.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_loop::runtime::{AgentRequest, AgentRunResult};
use common::AppError;
use contracts::ToolResult;
use contracts::chat::ChatTurnInput;

use super::host::OrchestratorExecutor;
use super::selected;
use super::types::{Channel, TaskBrief, WorkerHandoff};
use super::workers;

/// W2: channel iteration cap — total ReAct iterations one channel may burn
/// per user turn across all its briefs. Brief budgets (mode yaml, 4) still
/// apply per brief; the cap only binds multi-brief accumulation. When the
/// cap clamps a brief, the in-loop C5 budget-exhausted turn forces that
/// brief's handoff and the session seals.
pub const CHANNEL_ITERATION_CAP: u8 = 10;

/// Channel token budget — total tokens one channel may burn per user turn
/// across all briefs. Token-primary (aligns with BudgetConfig.max_tokens as
/// the loop's main stop); CHANNEL_ITERATION_CAP stays as a runaway safety
/// ceiling. Default mirrors rag mode max_tokens; tune per channel if needed.
pub const CHANNEL_TOKEN_CAP: u64 = 28_000;

/// Metadata key carrying the session's alias cursor into the loop run
/// (agent-loop seeds its counter from it; absent ⇒ 0, byte-identical to
/// pre-W1 single-brief behavior). Canonical def: `agent_loop::worker_contract`.
pub const ALIAS_START_METADATA: &str = agent_loop::worker_contract::RETRIEVAL_ALIAS_START_METADATA;

/// One delivered brief's retained record (drives resume compaction).
#[derive(Debug, Clone)]
pub struct BriefRecord {
    pub seq: u32,
    pub goal: String,
    pub summary: String,
    pub coverage: String,
    pub selected_aliases: Vec<u64>,
    /// The brief's final message (full handoff text — prose or JSON).
    pub final_message: String,
    pub tool_results: Vec<ToolResult>,
    pub iterations_used: u8,
}

/// What one `run_brief` yields to the dispatch layer.
#[derive(Debug)]
pub struct BriefOutcome {
    pub seq: u32,
    /// Raw run (kept for observability / store insertion).
    pub run: AgentRunResult,
    pub handoff: Option<WorkerHandoff>,
    /// Hydrated chunks from this brief's SELECTED log (offset-applied).
    pub hydrated: Vec<selected::HydratedChunk>,
    pub tool_results_delta: Vec<ToolResult>,
    pub iterations_used: u8,
    /// True when the channel cap clamped this brief (C5 forced handoff) —
    /// the session seals right after.
    pub cap_clamped: bool,
}

/// Why a brief was rejected before running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Channel cap already spent — session sealed.
    BudgetExhausted,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BudgetExhausted => write!(f, "channel budget exhausted"),
        }
    }
}

/// Per-channel persistent worker session (one turn).
#[derive(Debug)]
pub struct WorkerSession {
    pub channel: Channel,
    briefs: Vec<BriefRecord>,
    /// Retrieval-log alias cursor — aliases stay unique across briefs.
    alias_cursor: Arc<AtomicU64>,
    /// Cumulative iterations spent this turn (runaway safety ceiling).
    pub iterations_used: u8,
    /// Cumulative tokens spent this turn (drives the channel seal — token-primary).
    pub tokens_used: u64,
    sealed: bool,
    pub failed: bool,
}

impl WorkerSession {
    pub fn new(channel: Channel) -> Self {
        Self {
            channel,
            briefs: Vec::new(),
            alias_cursor: Arc::new(AtomicU64::new(0)),
            iterations_used: 0,
            tokens_used: 0,
            sealed: false,
            failed: false,
        }
    }

    pub fn briefs(&self) -> &[BriefRecord] {
        &self.briefs
    }

    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub fn next_seq(&self) -> u32 {
        self.briefs.len() as u32 + 1
    }

    /// Run ONE brief through the shared executor with the session's resume
    /// context injected. Sequential per session by construction (&mut self).
    pub async fn run_brief(
        &mut self,
        brief: &TaskBrief,
        base: &AgentRequest,
        executor: &dyn OrchestratorExecutor,
    ) -> Result<Result<BriefOutcome, AppError>, SessionError> {
        if self.sealed {
            return Err(SessionError::BudgetExhausted);
        }
        // Token-primary budget: seal when the channel token pool is spent.
        // Iterations remain as a runaway safety ceiling (anti-infinite-loop).
        let token_remaining = CHANNEL_TOKEN_CAP.saturating_sub(self.tokens_used);
        let iter_remaining = CHANNEL_ITERATION_CAP.saturating_sub(self.iterations_used);
        if token_remaining == 0 || iter_remaining == 0 {
            self.sealed = true;
            return Err(SessionError::BudgetExhausted);
        }
        let seq = self.next_seq();
        let mut req = base.clone();
        req.messages = resume_context_messages(&self.briefs);
        // Per-brief budget (mode yaml, 4/3) clamped by the remaining channel
        // cap; when the clamp binds, the loop's C5 budget-exhausted turn
        // forces this brief's handoff and the session seals right after.
        let per_brief = per_brief_budget(self.channel);
        let effective = per_brief.min(iter_remaining);
        let cap_clamped = effective < per_brief;
        req.max_iterations = Some(effective);
        let alias_before = self.alias_cursor.load(Ordering::Relaxed);
        if alias_before > 0 {
            req.metadata.insert(
                ALIAS_START_METADATA.into(),
                serde_json::json!(alias_before),
            );
        }

        let run = match executor.run_channel(self.channel, brief, &req).await {
            Ok(run) => run,
            Err(e) => {
                // Failure isolation: this session is poisoned; the dispatch
                // layer will replace it on the next brief.
                self.failed = true;
                return Ok(Err(e));
            }
        };

        let iterations = run
            .budget_used
            .as_ref()
            .map(|b| b.current)
            .unwrap_or(0);
        let delta = run.tool_results.clone();
        let handoff = workers::worker_handoff_from_run(&run);
        // Alias cursor advances by the chunks this brief emitted; hydration
        // of this brief's SELECTED log resolves against the pre-run offset.
        let aliased = selected::alias_chunks_in_order(&run.tool_results).len() as u64;
        let hydrated =
            hydrate_selected_offset(&run.answer, &run.tool_results, alias_before);
        self.alias_cursor.store(alias_before + aliased, Ordering::Relaxed);
        let tokens = run.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
        self.iterations_used = self.iterations_used.saturating_add(iterations);
        self.tokens_used = self.tokens_used.saturating_add(tokens);

        let summary = handoff
            .as_ref()
            .map(|h| h.summary.clone())
            .unwrap_or_default();
        let record = BriefRecord {
            seq,
            goal: brief.goal.clone(),
            summary: summary.clone(),
            coverage: handoff
                .as_ref()
                .map(|h| h.coverage.clone())
                .unwrap_or_else(|| "partial".to_string()),
            selected_aliases: selected::parse_selected_aliases(&run.answer),
            final_message: run.answer.clone(),
            tool_results: delta.clone(),
            iterations_used: iterations,
        };
        self.briefs.push(record);

        let token_exhausted = self.tokens_used >= CHANNEL_TOKEN_CAP;
        let iter_exhausted = self.iterations_used >= CHANNEL_ITERATION_CAP;
        // cap_clamped no longer seals on its own (token-primary design): a
        // clamped brief just truncates itself; the next brief runs if token
        // budget remains. Fixes the "half-loaded cannot top-up" symptom
        // (handover doc §5.1 / P0): CAP 10 vs per_brief 12 used to seal after
        // the first brief, blocking the second brief's evidence top-up.
        if token_exhausted || iter_exhausted {
            self.sealed = true;
        }
        Ok(Ok(BriefOutcome {
            seq,
            run,
            handoff,
            hydrated,
            tool_results_delta: delta,
            iterations_used: iterations,
            cap_clamped,
        }))
    }
}

/// Per-brief iteration budget by channel (mode yaml defaults: rag=4,
/// search=3). Uses the yaml `max_iterations` — tier overrides
/// (`by_user_tier`) are intentionally NOT consulted: worker briefs are
/// orchestrator-owned, not user-tiered (design §3.2 "沿用 mode yaml").
fn per_brief_budget(channel: Channel) -> u8 {
    let id = match channel {
        Channel::Rag => "rag",
        Channel::Search => "search",
    };
    agent_loop::load_mode_config(id)
        .map(|m| m.budget.max_iterations)
        .unwrap_or(4)
        .max(1)
}

/// W2 (design's BUG-1): hydrate a brief's SELECTED log against ITS OWN tool
/// delta — alias `#n` resolves to the (n - offset)-th chunk of this brief's
/// stream, never to an earlier brief's chunks.
pub fn hydrate_selected_offset(
    final_message: &str,
    tool_results: &[ToolResult],
    alias_offset: u64,
) -> Vec<selected::HydratedChunk> {
    let aliases = selected::parse_selected_aliases(final_message);
    if aliases.is_empty() {
        return Vec::new();
    }
    let stream = selected::alias_chunks_in_order(tool_results);
    let mut out: Vec<selected::HydratedChunk> = Vec::new();
    for alias in aliases {
        let Some(local) = alias.checked_sub(alias_offset).filter(|local| *local >= 1) else {
            tracing::warn!(alias, alias_offset, "SELECTED alias below session offset");
            continue;
        };
        let Some(chunk) = (local as usize).checked_sub(1).and_then(|idx| stream.get(idx)) else {
            tracing::warn!(alias, "SELECTED alias did not resolve to a retrieved chunk");
            continue;
        };
        if out.iter().any(|c| c.chunk_id == chunk.chunk_id) {
            continue;
        }
        out.push(chunk.clone());
    }
    out
}

/// Resume context for the next brief: older briefs ride as one synthetic
/// compaction entry each (goal + summary + coverage + SELECTED aliases); the
/// MOST RECENT brief carries its full handoff text so nothing the worker
/// just learned is lost. Empty for the first brief (pre-W1 behavior).
pub fn resume_context_messages(briefs: &[BriefRecord]) -> Vec<ChatTurnInput> {
    let mut out = Vec::new();
    let last = briefs.len().saturating_sub(1);
    for (idx, b) in briefs.iter().enumerate() {
        let content = if idx == last {
            format!(
                "[前序任务 {} 完整交接] 目标：{}\n交接消息（原文）：\n{}\n\
                 （这是同一通道上一任务的完整交接，请在此基础上继续；覆盖判断：{}；\
                 圈选证据编号：{}）",
                b.seq,
                b.goal,
                b.final_message,
                b.coverage,
                alias_list(&b.selected_aliases),
            )
        } else {
            format!(
                "[前序任务 {} 压缩] 目标：{}\n交接摘要：{}\n覆盖判断：{}\n圈选证据编号：{}",
                b.seq,
                b.goal,
                b.summary,
                b.coverage,
                alias_list(&b.selected_aliases),
            )
        };
        out.push(ChatTurnInput {
            role: "user".to_string(),
            content,
            resolved_query: None,
        });
    }
    out
}

fn alias_list(aliases: &[u64]) -> String {
    if aliases.is_empty() {
        return "（未圈选）".to_string();
    }
    aliases
        .iter()
        .map(|a| format!("#{a}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(seq: u32, goal: &str) -> BriefRecord {
        BriefRecord {
            seq,
            goal: goal.to_string(),
            summary: format!("摘要{seq}"),
            coverage: "partial".to_string(),
            selected_aliases: vec![1, 3],
            final_message: format!("交接原文{seq}"),
            tool_results: Vec::new(),
            iterations_used: 2,
        }
    }

    #[test]
    fn first_brief_has_no_resume_context() {
        assert!(resume_context_messages(&[]).is_empty());
    }

    #[test]
    fn older_briefs_compact_latest_keeps_full_text() {
        let briefs = vec![record(1, "任务一"), record(2, "任务二"), record(3, "任务三")];
        let msgs = resume_context_messages(&briefs);
        assert_eq!(msgs.len(), 3);
        assert!(msgs[0].content.contains("[前序任务 1 压缩]"));
        assert!(msgs[0].content.contains("摘要1"));
        assert!(!msgs[0].content.contains("交接原文1"));
        assert!(msgs[1].content.contains("[前序任务 2 压缩]"));
        // Most recent brief keeps the FULL handoff text.
        assert!(msgs[2].content.contains("[前序任务 3 完整交接]"));
        assert!(msgs[2].content.contains("交接原文3"));
        assert!(msgs[2].content.contains("#1, #3"));
    }

    #[test]
    fn hydration_respects_alias_offset() {
        let results = vec![ToolResult {
            tool: "dense_retrieval".into(),
            version: "1.0".into(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!([
                {"chunk_id": "c1", "text": "t1"},
                {"chunk_id": "c2", "text": "t2"},
            ])),
            trace: None,
        }];
        // Brief had alias cursor 5 before running: its chunks are #6 #7.
        let h = hydrate_selected_offset("SELECTED: #7", &results, 5);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].chunk_id, "c2");
        // Alias below the offset belongs to an earlier brief — not resolvable.
        assert!(hydrate_selected_offset("SELECTED: #5", &results, 5).is_empty());
        // Zero offset degrades to the plain hydration semantics.
        let h0 = hydrate_selected_offset("SELECTED: #1", &results, 0);
        assert_eq!(h0[0].chunk_id, "c1");
    }

    // ---- W1/W2: run_brief integration (mock executor) ------------------------

    use super::super::host::OrchestratorExecutor;
    use super::super::types::{Channel, TaskBrief};
    use agent_loop::runtime::AgentRequest;
    use common::AppError;
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
    use std::sync::Mutex;

    struct CapturingExec {
        requests: Mutex<Vec<AgentRequest>>,
        script: Mutex<Vec<Box<dyn FnMut(&mut AgentRunResult) + Send>>>,
        fail_next: Mutex<bool>,
    }

    impl CapturingExec {
        fn new() -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                script: Mutex::new(Vec::new()),
                fail_next: Mutex::new(false),
            }
        }
        fn push_script(&self, f: impl FnMut(&mut AgentRunResult) + Send + 'static) {
            self.script.lock().unwrap().push(Box::new(f));
        }
    }

    #[async_trait::async_trait]
    impl OrchestratorExecutor for CapturingExec {
        async fn run_channel(
            &self,
            _channel: Channel,
            _brief: &TaskBrief,
            base: &AgentRequest,
        ) -> Result<AgentRunResult, AppError> {
            self.requests.lock().unwrap().push(base.clone());
            if *self.fail_next.lock().unwrap() {
                *self.fail_next.lock().unwrap() = false;
                return Err(AppError::internal("transport boom"));
            }
            let mut r = AgentRunResult::default();
            r.answer = "交接摘要".to_string();
            if let Some(mut f) = self.script.lock().unwrap().pop() {
                f(&mut r);
            }
            Ok(r)
        }
        async fn run_chat(
            &self,
            _handoff: &super::super::types::ChatHandoff,
            _base: &AgentRequest,
            _sink: &dyn agent_loop::events::AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            Ok(AgentRunResult::default())
        }
    }

    fn base_req() -> AgentRequest {
        AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "q".into(),
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
            auth: AuthContext::new(UserId::from(uuid::Uuid::nil()), SubjectKind::User),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
        }
    }

    fn chunk_run(chunk_id: &str, answer: &str) -> impl FnMut(&mut AgentRunResult) + Send + 'static {
        let chunk_id = chunk_id.to_string();
        let answer = answer.to_string();
        move |r: &mut AgentRunResult| {
            r.answer = answer.clone();
            r.tool_results = vec![ToolResult {
                tool: "dense_retrieval".into(),
                version: "1.0".into(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": chunk_id, "doc_id": "d1", "text": "evidence", "score": 0.9}
                ])),
                trace: None,
            }];
        }
    }

    #[tokio::test]
    async fn second_brief_resumes_with_compacted_context_and_alias_offset() {
        let exec = CapturingExec::new();
        exec.push_script(Box::new(chunk_run("c1", "first handoff")));
        let mut session = WorkerSession::new(Channel::Rag);

        let o1 = session
            .run_brief(&TaskBrief::new("任务一"), &base_req(), &exec)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(o1.seq, 1);
        assert_eq!(session.briefs().len(), 1);
        // Brief#1 request is context-free (byte-identical to pre-W1).
        let reqs = exec.requests.lock().unwrap();
        assert!(reqs[0].messages.is_empty());
        assert!(!reqs[0].metadata.contains_key(ALIAS_START_METADATA));
        // effective = min(per_brief=12, iter_remaining=10) = 10 (clamped by
        // CHANNEL_ITERATION_CAP safety ceiling; token-primary seal is separate).
        assert_eq!(reqs[0].max_iterations, Some(10));
        drop(reqs);

        exec.push_script(Box::new(chunk_run("c2", "SELECTED: #2 用了第二条")));
        let o2 = session
            .run_brief(&TaskBrief::new("任务二"), &base_req(), &exec)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(o2.seq, 2);
        let reqs = exec.requests.lock().unwrap();
        // Resume context: brief#1 rides as the FULL-text entry.
        assert_eq!(reqs[1].messages.len(), 1);
        assert!(reqs[1].messages[0].content.contains("[前序任务 1 完整交接]"));
        assert!(reqs[1].messages[0].content.contains("任务一"));
        // Alias cursor carried over (brief#1 emitted 1 chunk → start at 1).
        assert_eq!(
            reqs[1].metadata.get(ALIAS_START_METADATA).and_then(|v| v.as_u64()),
            Some(1)
        );
        drop(reqs);
        // Hydration is offset-aware: SELECTED #2 → this brief's second chunk
        // (c2 is the only chunk of brief#2, at alias #2 = offset 1 + local 1).
        assert_eq!(o2.hydrated.len(), 1);
        assert_eq!(o2.hydrated[0].chunk_id, "c2");
    }

    #[tokio::test]
    async fn hard_failure_marks_session_failed_for_isolation() {
        let exec = CapturingExec::new();
        *exec.fail_next.lock().unwrap() = true;
        let mut session = WorkerSession::new(Channel::Rag);
        let outcome = session
            .run_brief(&TaskBrief::new("任务"), &base_req(), &exec)
            .await
            .unwrap();
        assert!(outcome.is_err());
        assert!(session.failed, "transport failure poisons the session");
    }

    #[tokio::test]
    async fn cap_clamps_third_brief_and_seals_the_session() {
        let exec = CapturingExec::new();
        let mut session = WorkerSession::new(Channel::Rag);
        for _ in 0..2 {
            exec.push_script(Box::new(|r: &mut AgentRunResult| {
                r.budget_used = Some(agent_loop::runtime::BudgetUsage {
                    current: 4,
                    max: 4,
                });
            }));
            session
                .run_brief(&TaskBrief::new("b"), &base_req(), &exec)
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(session.iterations_used, 8);
        assert!(!session.is_sealed());

        // Third brief: remaining cap = 2 < per-brief 4 → clamped, then sealed.
        exec.push_script(Box::new(|r: &mut AgentRunResult| {
            r.budget_used = Some(agent_loop::runtime::BudgetUsage {
                current: 2,
                max: 2,
            });
        }));
        let o3 = session
            .run_brief(&TaskBrief::new("b3"), &base_req(), &exec)
            .await
            .unwrap()
            .unwrap();
        assert!(o3.cap_clamped, "cap clamps the brief budget");
        assert_eq!(
            exec.requests.lock().unwrap().last().unwrap().max_iterations,
            Some(2)
        );
        assert!(session.is_sealed(), "session seals after the clamped brief");

        // Fourth brief rejected with the budget-exhausted signal.
        let rejected = session
            .run_brief(&TaskBrief::new("b4"), &base_req(), &exec)
            .await;
        assert!(matches!(rejected, Err(SessionError::BudgetExhausted)));
    }

    #[tokio::test]
    async fn e105_scopes_to_the_briefs_own_retrieval_calls() {
        // BUG-1: brief#1 retrieved; brief#2 made ZERO new retrieval calls and
        // declares insufficient → E105 fires on brief#2's own delta.
        let exec = CapturingExec::new();
        let mut session = WorkerSession::new(Channel::Rag);
        exec.push_script(Box::new(chunk_run("c1", "first")));
        session
            .run_brief(&TaskBrief::new("b1"), &base_req(), &exec)
            .await
            .unwrap()
            .unwrap();

        // brief#2: no tool results at all + insufficient JSON.
        exec.push_script(Box::new(|r: &mut AgentRunResult| {
            r.answer = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到","coverage":"insufficient","gaps":[]}"#.to_string();
        }));
        let o2 = session
            .run_brief(&TaskBrief::new("b2"), &base_req(), &exec)
            .await
            .unwrap()
            .unwrap();
        let h = o2.handoff.expect("handoff");
        assert!(h.handoff_degraded, "E105 must fire on the brief's own zero-call delta");
        assert!(
            h.compile_diagnostics.contains(&"E105".to_string()),
            "{:?}",
            h.compile_diagnostics
        );
    }
}
