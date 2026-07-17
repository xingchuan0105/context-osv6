//! Core types for orchestrator + channel workers + chat exit (design 2026-07-16).

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// Worker output consumed by orchestrator / chat synthesize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePack {
    pub channel: Channel,
    pub status: PackStatus,
    pub dispatch_id: String,
    pub task_brief: TaskBrief,
    #[serde(default)]
    pub items: Vec<EvidenceItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Ledger entry for §7.2 completion invariant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchRecord {
    pub channel: Channel,
    pub dispatch_id: String,
    pub status: PackStatus,
}

impl From<&EvidencePack> for DispatchRecord {
    fn from(pack: &EvidencePack) -> Self {
        Self {
            channel: pack.channel,
            dispatch_id: pack.dispatch_id.clone(),
            status: pack.status,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatHandoff {
    pub mode: ChatExitMode,
    pub user_query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    #[serde(default)]
    pub packs: Vec<EvidencePack>,
    #[serde(default)]
    pub partial_notices: Vec<String>,
}
