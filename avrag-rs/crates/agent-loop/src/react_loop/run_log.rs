//! Structured per-run event log for the lead_workers lane.
//!
//! Discipline (borrowed from deepseek-harness `session/event` and the pi
//! session tree):
//!
//! - **Append-only events, not derived state.** Each entry records what
//!   happened (plan proposed, wave completed, pack superseded) with a
//!   monotonic seq and elapsed ms — never a recomputed aggregate label.
//! - **Surface is explicit, log-only is the default.** Only
//!   [`RunEventKind::surface`] kinds may be projected into model context
//!   (`[retrieval_worklog]`); counts, gate internals and per-call traces stay
//!   in telemetry / debug artifacts.
//! - **Abandoned paths leave a trace.** A re-brief wave does not silently
//!   replace the first wave: [`RunEventKind::PackSuperseded`] keeps the
//!   superseded attempt visible to the Lead, like a branch summary.

use serde::Serialize;
use std::time::Instant;

/// One brief as seen by the Lead planner (id / channel / objective only).
#[derive(Debug, Clone, Serialize)]
pub struct PlanBriefSummary {
    pub id: String,
    pub source: String,
    pub objective: String,
}

/// What happened, in run order. `#[serde(tag = "kind")]` keeps the JSONL-ish
/// debug dump self-describing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunEventKind {
    /// Lead plan accepted (or host fallback used). `briefs` covers retrieval,
    /// base_tools and none entries alike.
    PlanProposed {
        used_host_fallback: bool,
        briefs: Vec<PlanBriefSummary>,
    },
    /// Plan failed parse / structural gate → host asked Lead to re-plan.
    /// `raw_preview` (log-only) keeps the head of the rejected raw plan output
    /// so diagnosis can see the actual wire shape instead of guessing.
    PlanRepairRequested { reason: String, raw_preview: String },
    /// Brief rejected at the dispatch gate (never spawned a Worker).
    BriefRejected {
        id: String,
        source: String,
        reason: String,
    },
    /// Host BASE leaf executed (weather / calculator / user_context).
    BaseToolExecuted { tool: String, ok: bool },
    /// One tool call inside a Worker wave (from the outcome trace).
    ToolCall {
        wave: u8,
        channel: String,
        tool: String,
        ok: bool,
        elapsed_ms: Option<u64>,
        /// Error / degrade preview, empty on clean Ok.
        preview: String,
    },
    /// Worker returned a pack. `n_evidence` is a log-only diagnostic count;
    /// no digest of the evidence is recorded anywhere (see EvidencePack).
    WorkerCompleted {
        wave: u8,
        sub_task_id: String,
        channel: String,
        objective: String,
        n_evidence: usize,
        gaps: String,
    },
    /// PackGate outcome for one pack (host-side rewrite details).
    PackGated {
        wave: u8,
        channel: String,
        outcome: String,
    },
    /// A later wave replaced this channel's earlier pack (branch-summary
    /// analogue: the abandoned attempt stays on record).
    PackSuperseded { wave: u8, channel: String },
    /// Host structural re-brief wave dispatched (targets = sub-task/facet ids).
    RebriefWave { targets: Vec<String> },
    /// Retrieve leg closed, control handed to synthesis.
    Handoff { packs: usize },
}

/// One log entry: monotonic seq + ms since run start + kind.
#[derive(Debug, Clone, Serialize)]
pub struct RunEvent {
    pub seq: u32,
    pub at_ms: u64,
    #[serde(flatten)]
    pub event: RunEventKind,
}

impl RunEventKind {
    /// Surface events are the only kinds eligible for model-context
    /// projection (`[retrieval_worklog]`). Everything else is log-only
    /// (telemetry / debug artifact).
    pub fn surface(&self) -> bool {
        matches!(
            self,
            Self::PlanProposed { .. }
                | Self::PlanRepairRequested { .. }
                | Self::BriefRejected { .. }
                | Self::WorkerCompleted { .. }
                | Self::PackSuperseded { .. }
                | Self::RebriefWave { .. }
        )
    }
}

/// Append-only per-run log. Cheap: all payloads are already in memory.
#[derive(Debug)]
pub struct RunEventLog {
    started: Instant,
    events: Vec<RunEvent>,
}

impl Default for RunEventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl RunEventLog {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: RunEventKind) {
        let seq = self.events.len() as u32;
        let at_ms = self.started.elapsed().as_millis() as u64;
        self.events.push(RunEvent { seq, at_ms, event });
    }

    pub fn events(&self) -> &[RunEvent] {
        &self.events
    }

    /// Surface events in run order — the single source for the
    /// `[retrieval_worklog]` projection.
    pub fn surface_events(&self) -> impl Iterator<Item = &RunEvent> {
        self.events.iter().filter(|e| e.event.surface())
    }

    /// Full log (surface + log-only) for telemetry / debug artifacts.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.events).unwrap_or_else(|_| serde_json::json!([]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_and_surface_partition() {
        let mut log = RunEventLog::new();
        log.push(RunEventKind::PlanProposed {
            used_host_fallback: false,
            briefs: vec![PlanBriefSummary {
                id: "t1".into(),
                source: "rag".into(),
                objective: "找定义".into(),
            }],
        });
        log.push(RunEventKind::ToolCall {
            wave: 0,
            channel: "rag".into(),
            tool: "dense_retrieval".into(),
            ok: true,
            elapsed_ms: Some(12),
            preview: String::new(),
        });
        log.push(RunEventKind::RebriefWave {
            targets: vec!["rag".into()],
        });
        assert_eq!(log.events().len(), 3);
        assert_eq!(log.events()[2].seq, 2);
        let surface: Vec<_> = log.surface_events().collect();
        assert_eq!(surface.len(), 2, "ToolCall is log-only");
        assert!(matches!(
            surface[1].event,
            RunEventKind::RebriefWave { .. }
        ));
        // JSON roundtrip sanity: kind tag present.
        let v = log.to_json();
        assert_eq!(v[0]["kind"], "plan_proposed");
        assert_eq!(v[1]["kind"], "tool_call");
    }
}
