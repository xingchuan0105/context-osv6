//! Task Brief (Lead → Worker) — `task_brief_v1`.

use serde::{Deserialize, Serialize};

/// Channel / tool routing intent for one sub-task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferredSource {
    Rag,
    Web,
    /// BASE tools only (weather / calculator / …); no retrieval Worker.
    BaseTools,
    /// No Worker; Lead may close without retrieval packs.
    None,
}

impl PreferredSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rag => "rag",
            Self::Web => "web",
            Self::BaseTools => "base_tools",
            Self::None => "none",
        }
    }

    /// Whether host should materialize a retrieval Worker for this brief.
    pub fn spawns_retrieval_worker(self) -> bool {
        matches!(self, Self::Rag | Self::Web)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubTask {
    pub id: String,
    pub objective: String,
    /// Boundaries prose (prompts; not host-parsed beyond non-empty check).
    #[serde(default)]
    pub boundaries: String,
    pub preferred_source: PreferredSource,
    /// Web host fan-out queries; empty ⇒ host uses `original_query` alone.
    #[serde(default)]
    pub queries: Vec<String>,
    /// Worker inner steps (SaC turns). Host clamps to [1, 5].
    #[serde(default = "default_max_steps")]
    pub max_steps: u8,
    #[serde(default)]
    pub success_criteria: String,
}

fn default_max_steps() -> u8 {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskBrief {
    #[serde(default = "task_brief_schema")]
    pub schema_version: String,
    pub original_query: String,
    #[serde(default)]
    pub conversation_context_summary: String,
    pub sub_task: SubTask,
    #[serde(default = "evidence_pack_schema_name")]
    pub output_schema: String,
    #[serde(default)]
    pub grounding_rule: String,
}

fn task_brief_schema() -> String {
    "task_brief_v1".into()
}

fn evidence_pack_schema_name() -> String {
    "evidence_pack_v1".into()
}

/// Host structural failure before spawning a Worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskBriefGateError {
    EmptyOriginalQuery,
    EmptyObjective,
    SourceNotActivated { source: PreferredSource },
    MaxStepsOutOfRange { got: u8 },
    SchemaMismatch { got: String },
}

impl std::fmt::Display for TaskBriefGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyOriginalQuery => write!(f, "original_query empty"),
            Self::EmptyObjective => write!(f, "sub_task.objective empty"),
            Self::SourceNotActivated { source } => {
                write!(f, "preferred_source {:?} not in activated caps", source)
            }
            Self::MaxStepsOutOfRange { got } => {
                write!(f, "max_steps {got} not in [1,5]")
            }
            Self::SchemaMismatch { got } => write!(f, "schema_version {got:?}"),
        }
    }
}

/// Activated product capabilities for the turn (from `capabilities[]`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ActivatedCaps {
    pub rag: bool,
    pub search: bool,
}

/// Structural start gate for a Task Brief.
///
/// - `base_tools` / `none` always pass the source check (no retrieval Worker).
/// - `rag` requires `caps.rag`; `web` requires `caps.search`.
pub fn validate_task_brief(
    brief: &TaskBrief,
    caps: ActivatedCaps,
) -> Result<(), TaskBriefGateError> {
    if brief.schema_version != "task_brief_v1" {
        return Err(TaskBriefGateError::SchemaMismatch {
            got: brief.schema_version.clone(),
        });
    }
    if brief.original_query.trim().is_empty() {
        return Err(TaskBriefGateError::EmptyOriginalQuery);
    }
    if brief.sub_task.objective.trim().is_empty() {
        return Err(TaskBriefGateError::EmptyObjective);
    }
    let steps = brief.sub_task.max_steps;
    if !(1..=5).contains(&steps) {
        return Err(TaskBriefGateError::MaxStepsOutOfRange { got: steps });
    }
    match brief.sub_task.preferred_source {
        PreferredSource::Rag if !caps.rag => {
            return Err(TaskBriefGateError::SourceNotActivated {
                source: PreferredSource::Rag,
            });
        }
        PreferredSource::Web if !caps.search => {
            return Err(TaskBriefGateError::SourceNotActivated {
                source: PreferredSource::Web,
            });
        }
        PreferredSource::Rag
        | PreferredSource::Web
        | PreferredSource::BaseTools
        | PreferredSource::None => {}
    }
    Ok(())
}

/// Effective web queries for host fan-out (non-empty trimmed strings).
pub fn effective_web_queries(brief: &TaskBrief) -> Vec<String> {
    let mut q: Vec<String> = brief
        .sub_task
        .queries
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if q.is_empty() {
        let o = brief.original_query.trim();
        if !o.is_empty() {
            q.push(o.to_string());
        }
    }
    // Soft product cap (design §6.2): at most 5.
    q.truncate(5);
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_brief(source: PreferredSource) -> TaskBrief {
        TaskBrief {
            schema_version: "task_brief_v1".into(),
            original_query: "什么是 BYOK".into(),
            conversation_context_summary: String::new(),
            sub_task: SubTask {
                id: "t1".into(),
                objective: "检索 BYOK 定义".into(),
                boundaries: "只检索".into(),
                preferred_source: source,
                queries: vec![],
                max_steps: 4,
                success_criteria: "有定义句".into(),
            },
            output_schema: "evidence_pack_v1".into(),
            grounding_rule: "仅 observation".into(),
        }
    }

    #[test]
    fn validates_rag_when_cap_on() {
        let b = sample_brief(PreferredSource::Rag);
        assert!(validate_task_brief(&b, ActivatedCaps { rag: true, search: false }).is_ok());
    }

    #[test]
    fn rejects_rag_when_cap_off() {
        let b = sample_brief(PreferredSource::Rag);
        let err = validate_task_brief(&b, ActivatedCaps { rag: false, search: true }).unwrap_err();
        assert!(matches!(
            err,
            TaskBriefGateError::SourceNotActivated {
                source: PreferredSource::Rag
            }
        ));
    }

    #[test]
    fn base_tools_ok_without_retrieval_caps() {
        let b = sample_brief(PreferredSource::BaseTools);
        assert!(validate_task_brief(&b, ActivatedCaps::default()).is_ok());
        assert!(!PreferredSource::BaseTools.spawns_retrieval_worker());
    }

    #[test]
    fn max_steps_range() {
        let mut b = sample_brief(PreferredSource::Web);
        b.sub_task.max_steps = 0;
        assert!(matches!(
            validate_task_brief(&b, ActivatedCaps { rag: false, search: true }),
            Err(TaskBriefGateError::MaxStepsOutOfRange { got: 0 })
        ));
        b.sub_task.max_steps = 6;
        assert!(matches!(
            validate_task_brief(&b, ActivatedCaps { rag: false, search: true }),
            Err(TaskBriefGateError::MaxStepsOutOfRange { got: 6 })
        ));
    }

    #[test]
    fn effective_queries_fallback_and_cap() {
        let mut b = sample_brief(PreferredSource::Web);
        assert_eq!(effective_web_queries(&b), vec!["什么是 BYOK".to_string()]);
        b.sub_task.queries = (1..=8).map(|i| format!("q{i}")).collect();
        let q = effective_web_queries(&b);
        assert_eq!(q.len(), 5);
        assert_eq!(q[0], "q1");
    }

    #[test]
    fn serde_roundtrip() {
        let b = sample_brief(PreferredSource::Web);
        let v = serde_json::to_value(&b).unwrap();
        let back: TaskBrief = serde_json::from_value(v).unwrap();
        assert_eq!(back, b);
    }
}
