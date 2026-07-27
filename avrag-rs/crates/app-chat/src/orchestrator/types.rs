//! Core types for orchestrator + channel workers + chat exit (design 2026-07-16,
//! evidence-store revision 2026-07-18).

use serde::{Deserialize, Serialize};

use super::store::{EvidenceEntry, EvidenceListing, SourceDoc};

/// Product channel that can be materialized from `capabilities[]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Rag,
    Search,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Rag => "rag",
            Channel::Search => "search",
        }
    }
}

/// Brief from orchestrator → worker (`goal` required).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBrief {
    pub goal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u32>,
}

impl TaskBrief {
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            constraints: Vec::new(),
            focus_terms: Vec::new(),
            max_items: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackStatus {
    Ok,
    Empty,
    Error,
}

/// Ledger entry for §7.2 completion invariant + turn metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchRecord {
    pub channel: Channel,
    pub dispatch_id: String,
    pub status: PackStatus,
    /// Evidence entries inserted into the store by this dispatch.
    pub item_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One claim from a worker handoff, optionally grounded on evidence pointers
/// (store `E{n}`, chunk ids, or free-form locators the worker had in view).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerKeyFact {
    pub claim: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

/// Structured worker exit contract (V2 design §3.4).
///
/// Replaces free-form digest notes so coverage gaps are visible to the
/// orchestrator and chat exit. Workers are prompted to emit this JSON;
/// free-form answers fall back to `summary` + `coverage = "partial"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerHandoff {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_facts: Vec<WorkerKeyFact>,
    /// `full` | `partial` | `insufficient` (open string; unknown treated as partial).
    pub coverage: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<String>,
    /// C4: true when the worker's final message failed structured-handoff
    /// validation or content sanitization (unparsable output, fabricated
    /// `<code_execution_result>` blocks, evidence pointers absent from the
    /// worker's recorded tool results). Downstream should treat the handoff
    /// as untrustworthy.
    #[serde(default)]
    pub handoff_degraded: bool,
}

impl WorkerHandoff {
    /// C4: deterministic fallback when the worker's final message is not a
    /// parseable handoff at all. The raw text is deliberately NOT carried
    /// over (q087 raw code block / q039 fabricated execution-result would
    /// otherwise be rendered into the Answer's channel outcomes).
    pub fn degraded_unparsable() -> Self {
        Self {
            summary: "worker output unparsable as handoff JSON".to_string(),
            key_facts: Vec::new(),
            coverage: "insufficient".into(),
            gaps: Vec::new(),
            handoff_degraded: true,
        }
    }

    pub fn freeform_summary(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            key_facts: Vec::new(),
            coverage: "partial".into(),
            gaps: Vec::new(),
            handoff_degraded: false,
        }
    }

    pub fn is_full_coverage(&self) -> bool {
        self.coverage.eq_ignore_ascii_case("full") && self.gaps.is_empty()
    }
}

/// Per-channel worker outcome handed to the chat exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelNote {
    pub channel: Channel,
    pub status: PackStatus,
    pub item_count: usize,
    /// Flat summary (always mirrors `handoff.summary` when structured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Structured handoff when the worker produced one (or freeform fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<WorkerHandoff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ChannelNote {
    pub fn with_handoff(
        channel: Channel,
        status: PackStatus,
        item_count: usize,
        handoff: Option<WorkerHandoff>,
        error: Option<String>,
    ) -> Self {
        let note = handoff.as_ref().map(|h| h.summary.clone());
        Self {
            channel,
            status,
            item_count,
            note,
            handoff,
            error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatExitMode {
    Direct,
    Synthesize,
}

/// Handoff orchestrator → chat agent (Option B sole user-facing exit).
///
/// Carries evidence **by reference** (store listings / eids) plus worker
/// digests — never raw chunk dumps (design §3.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatHandoff {
    pub mode: ChatExitMode,
    pub user_query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// Source documents in scope (identity for genre judgment).
    #[serde(default)]
    pub source_docs: Vec<SourceDoc>,
    /// Evidence stubs (`E{n}` + label + preview) the chat may cite.
    #[serde(default)]
    pub listings: Vec<EvidenceListing>,
    /// Targeted doc-orientation entries (full text; orientation only, never citable).
    #[serde(default)]
    pub targeted: Vec<EvidenceEntry>,
    /// Per-channel worker outcomes (summary + status).
    #[serde(default)]
    pub channel_notes: Vec<ChannelNote>,
    #[serde(default)]
    pub partial_notices: Vec<String>,
}
