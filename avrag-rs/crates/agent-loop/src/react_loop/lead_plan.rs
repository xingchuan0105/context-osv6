//! Lead LLM plan → TaskBrief list (host fallback on failure).
//!
//! Design: `docs/plans/2026-08-11-lead-rag-web-workers-design.md` D1 / §2.4.

use avrag_llm::{ChatMessage, LlmClient, LlmUsage};
use serde::Deserialize;

use super::json_fence;
use crate::lead_workers::{
    ActivatedCaps, PreferredSource, SubTask, TaskBrief, validate_task_brief,
};
use crate::runtime::AgentRequest;

const PLAN_SYSTEM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/pipeline/lead-plan.system.md"
));
const PLAN_REPAIR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/pipeline/lead-plan-repair.md"
));
const PLAN_USER_TMPL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/pipeline/lead-plan.user.tmpl.md"
));
const DEFAULT_BOUNDARIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/workers/default-boundaries.md"
));
const DEFAULT_GROUNDING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/workers/default-grounding.md"
));

/// Outcome of plan parse — distinguishes empty retrieval plan (BASE-only) from hard fail.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanParseOutcome {
    /// Retrieval and/or base_tools briefs. Empty retrieval = no Worker channels.
    Ok {
        retrieval_briefs: Vec<TaskBrief>,
        base_tool_briefs: Vec<TaskBrief>,
    },
    /// JSON/schema unusable → host should fall back to default retrieval briefs.
    Invalid,
}

/// Lead planner result including billable usage.
#[derive(Debug, Clone)]
pub struct LeadPlanResult {
    pub retrieval_briefs: Vec<TaskBrief>,
    pub base_tool_briefs: Vec<TaskBrief>,
    pub usage: LlmUsage,
    /// True when host fallback briefs were used (LLM fail / invalid plan).
    pub used_host_fallback: bool,
}

#[derive(Debug, Deserialize)]
struct PlanWire {
    #[serde(default)]
    original_query: String,
    #[serde(default)]
    conversation_context_summary: String,
    #[serde(default)]
    briefs: Vec<BriefWire>,
}

#[derive(Debug, Deserialize)]
struct BriefWire {
    #[serde(default)]
    id: String,
    #[serde(default)]
    objective: String,
    #[serde(default)]
    preferred_source: String,
    #[serde(default)]
    queries: Vec<String>,
    #[serde(default)]
    max_steps: Option<u8>,
    #[serde(default)]
    success_criteria: String,
    #[serde(default)]
    boundaries: String,
}

/// Call Lead planner. On LLM failure or invalid JSON → `host_fallback` retrieval.
/// On valid plan with only base_tools/none → empty retrieval + optional base briefs.
pub async fn fetch_lead_briefs(
    llm: &LlmClient,
    request: &AgentRequest,
    caps: ActivatedCaps,
    plan_context_obs: &str,
    host_fallback: Vec<TaskBrief>,
) -> LeadPlanResult {
    let temperature = 0.3_f32;
    let mut usage = LlmUsage::zeroed();
    let history_block = format_history_block(request);
    let user = PLAN_USER_TMPL
        .replace("{plan_context_obs}", plan_context_obs)
        .replace("{history_block}", &history_block)
        .replace("{query}", request.query.trim());
    let messages = vec![
        ChatMessage::system(trim_md(PLAN_SYSTEM)),
        ChatMessage::user(user),
    ];
    let Ok(response) = llm.complete_json_mode(&messages, Some(temperature)).await else {
        tracing::warn!("lead_plan llm failed; using host fallback briefs");
        return LeadPlanResult {
            retrieval_briefs: host_fallback,
            base_tool_briefs: vec![],
            usage,
            used_host_fallback: true,
        };
    };
    usage.accumulate(&response.usage);
    match parse_and_validate(&response.content, request.query.trim(), caps) {
        PlanParseOutcome::Ok {
            retrieval_briefs,
            base_tool_briefs,
        } => {
            return LeadPlanResult {
                retrieval_briefs,
                base_tool_briefs,
                usage,
                used_host_fallback: false,
            };
        }
        PlanParseOutcome::Invalid => {}
    }

    let first_err = classify_parse_error(&response.content);
    let repair = PLAN_REPAIR.replace("{parse_error}", &first_err);
    let repair_messages = vec![
        ChatMessage::system(trim_md(PLAN_SYSTEM)),
        ChatMessage::assistant(response.content.as_str()),
        ChatMessage::user(repair),
    ];
    let Ok(repaired) = llm
        .complete_json_mode(&repair_messages, Some(temperature))
        .await
    else {
        return LeadPlanResult {
            retrieval_briefs: host_fallback,
            base_tool_briefs: vec![],
            usage,
            used_host_fallback: true,
        };
    };
    usage.accumulate(&repaired.usage);
    match parse_and_validate(&repaired.content, request.query.trim(), caps) {
        PlanParseOutcome::Ok {
            retrieval_briefs,
            base_tool_briefs,
        } => LeadPlanResult {
            retrieval_briefs,
            base_tool_briefs,
            usage,
            used_host_fallback: false,
        },
        PlanParseOutcome::Invalid => LeadPlanResult {
            retrieval_briefs: host_fallback,
            base_tool_briefs: vec![],
            usage,
            used_host_fallback: true,
        },
    }
}

fn trim_md(s: &str) -> &str {
    // Strip YAML frontmatter if present.
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("---") {
        if let Some(idx) = rest.find("\n---") {
            return rest[idx + 4..].trim();
        }
    }
    t
}

fn format_history_block(request: &AgentRequest) -> String {
    let mut lines = Vec::new();
    // Recent prior turns (skip empty); cap for plan context size (rounds budget, not token wall).
    for turn in request
        .messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let role = turn.role.trim();
        let content = turn.content.trim();
        if content.is_empty() {
            continue;
        }
        let snippet: String = content.chars().take(400).collect();
        lines.push(format!("- [{role}] {snippet}"));
    }
    if lines.is_empty() {
        "对话历史：本回合无 prior 用户/助手消息。".into()
    } else {
        format!("对话历史（近序）：\n{}", lines.join("\n"))
    }
}

fn classify_parse_error(raw: &str) -> String {
    let stripped = json_fence::strip_json_fence(raw);
    match serde_json::from_str::<serde_json::Value>(&stripped) {
        Err(e) => e.to_string(),
        Ok(_) => "plan_schema_or_source_invalid".into(),
    }
}

/// Parse plan JSON. `Ok { retrieval_briefs: [] }` means intentional no-retrieval.
pub fn parse_and_validate(raw: &str, query: &str, caps: ActivatedCaps) -> PlanParseOutcome {
    let stripped = json_fence::strip_json_fence(raw);
    let wire: PlanWire = match serde_json::from_str(&stripped) {
        Ok(w) => w,
        Err(_) => return PlanParseOutcome::Invalid,
    };
    let original = if wire.original_query.trim().is_empty() {
        query.trim().to_string()
    } else {
        wire.original_query.trim().to_string()
    };
    if original.is_empty() {
        return PlanParseOutcome::Invalid;
    }
    if wire.briefs.is_empty() {
        // Empty briefs array is invalid — cannot tell BASE-only from broken plan.
        return PlanParseOutcome::Invalid;
    }

    let ctx_summary = wire.conversation_context_summary.trim().to_string();
    let mut retrieval = Vec::new();
    let mut base_tools = Vec::new();
    let mut saw_any_valid = false;
    let mut saw_none_only = false;
    // v1 PlanGate: at most one retrieval brief per channel (first wins).
    let mut saw_rag_retrieval = false;
    let mut saw_web_retrieval = false;
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, b) in wire.briefs.into_iter().take(5).enumerate() {
        let Some(source) = parse_source(&b.preferred_source) else {
            // Skip unknown source entry; do not fail whole plan.
            continue;
        };
        match source {
            PreferredSource::Rag if !caps.rag => continue,
            PreferredSource::Web if !caps.search => continue,
            _ => {}
        }
        saw_any_valid = true;

        let id = if b.id.trim().is_empty() {
            format!("t{}", i + 1)
        } else {
            b.id.trim().to_string()
        };
        if !seen_ids.insert(id.clone()) {
            tracing::warn!(sub_task_id = %id, "lead_plan: duplicate sub_task.id dropped");
            continue;
        }

        if source == PreferredSource::None {
            saw_none_only = true;
            continue;
        }

        match source {
            PreferredSource::Rag if saw_rag_retrieval => {
                tracing::warn!(
                    sub_task_id = %id,
                    "lead_plan: extra rag brief dropped (one per channel)"
                );
                continue;
            }
            PreferredSource::Web if saw_web_retrieval => {
                tracing::warn!(
                    sub_task_id = %id,
                    "lead_plan: extra web brief dropped (one per channel)"
                );
                continue;
            }
            PreferredSource::Rag => saw_rag_retrieval = true,
            PreferredSource::Web => saw_web_retrieval = true,
            PreferredSource::BaseTools | PreferredSource::None => {}
        }

        let objective = if b.objective.trim().is_empty() {
            original.clone()
        } else {
            b.objective.trim().to_string()
        };
        let max_steps = b.max_steps.unwrap_or(4).clamp(1, 5);
        let brief = TaskBrief {
            schema_version: "task_brief_v1".into(),
            original_query: original.clone(),
            conversation_context_summary: ctx_summary.clone(),
            sub_task: SubTask {
                id,
                objective,
                boundaries: if b.boundaries.trim().is_empty() {
                    trim_md(DEFAULT_BOUNDARIES).to_string()
                } else {
                    b.boundaries
                },
                preferred_source: source,
                queries: b.queries,
                max_steps,
                success_criteria: b.success_criteria,
            },
            output_schema: "evidence_pack_v1".into(),
            grounding_rule: trim_md(DEFAULT_GROUNDING).to_string(),
        };
        if validate_task_brief(&brief, caps).is_err() {
            continue;
        }
        match source {
            PreferredSource::Rag | PreferredSource::Web => retrieval.push(brief),
            PreferredSource::BaseTools => base_tools.push(brief),
            PreferredSource::None => {}
        }
    }

    if !saw_any_valid {
        return PlanParseOutcome::Invalid;
    }
    // Valid plan that only uses base_tools/none → empty retrieval (P0-1 short path).
    if retrieval.is_empty() && (!base_tools.is_empty() || saw_none_only) {
        return PlanParseOutcome::Ok {
            retrieval_briefs: vec![],
            base_tool_briefs: base_tools,
        };
    }
    if retrieval.is_empty() && base_tools.is_empty() {
        return PlanParseOutcome::Invalid;
    }
    PlanParseOutcome::Ok {
        retrieval_briefs: retrieval,
        base_tool_briefs: base_tools,
    }
}

fn parse_source(s: &str) -> Option<PreferredSource> {
    match s.trim().to_ascii_lowercase().as_str() {
        "rag" => Some(PreferredSource::Rag),
        "web" => Some(PreferredSource::Web),
        "base_tools" | "base" => Some(PreferredSource::BaseTools),
        "none" => Some(PreferredSource::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_plan() {
        let raw = r#"{
          "original_query": "什么是 BYOK",
          "briefs": [
            {"id":"t1","objective":"网页查 BYOK","preferred_source":"web","queries":["BYOK"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: false,
            search: true,
        };
        match parse_and_validate(raw, "q", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs, ..
            } => {
                assert_eq!(retrieval_briefs.len(), 1);
                assert_eq!(
                    retrieval_briefs[0].sub_task.preferred_source,
                    PreferredSource::Web
                );
            }
            PlanParseOutcome::Invalid => panic!("expected ok"),
        }
    }

    #[test]
    fn drops_inactive_source() {
        let raw = r#"{
          "original_query": "x",
          "briefs": [
            {"id":"t1","objective":"kb","preferred_source":"rag","max_steps":3},
            {"id":"t2","objective":"web","preferred_source":"web","queries":["x"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: false,
        };
        match parse_and_validate(raw, "x", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs, ..
            } => {
                assert_eq!(retrieval_briefs.len(), 1);
                assert_eq!(
                    retrieval_briefs[0].sub_task.preferred_source,
                    PreferredSource::Rag
                );
            }
            PlanParseOutcome::Invalid => panic!("expected ok"),
        }
    }

    #[test]
    fn base_tools_only_is_ok_empty_retrieval() {
        let raw = r#"{
          "original_query": "北京今天天气",
          "briefs": [
            {"id":"t1","objective":"查天气","preferred_source":"base_tools","max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: true,
        };
        match parse_and_validate(raw, "北京今天天气", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs,
                base_tool_briefs,
            } => {
                assert!(
                    retrieval_briefs.is_empty(),
                    "BASE-only must not force retrieval workers"
                );
                assert_eq!(base_tool_briefs.len(), 1);
            }
            PlanParseOutcome::Invalid => panic!("BASE-only plan must be Ok empty, not Invalid"),
        }
    }

    #[test]
    fn unknown_source_skipped_not_whole_fail() {
        let raw = r#"{
          "original_query": "x",
          "briefs": [
            {"id":"t0","objective":"bad","preferred_source":"martian","max_steps":1},
            {"id":"t1","objective":"web","preferred_source":"web","queries":["x"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: false,
            search: true,
        };
        match parse_and_validate(raw, "x", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs, ..
            } => {
                assert_eq!(retrieval_briefs.len(), 1);
            }
            PlanParseOutcome::Invalid => panic!("should skip unknown source"),
        }
    }

    #[test]
    fn preserves_conversation_context_summary() {
        let raw = r#"{
          "original_query": "明天呢",
          "conversation_context_summary": "用户在问上海天气",
          "briefs": [
            {"id":"t1","objective":"上海明天天气","preferred_source":"base_tools"}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: false,
            search: false,
        };
        match parse_and_validate(raw, "明天呢", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs,
                base_tool_briefs,
            } => {
                assert!(retrieval_briefs.is_empty());
                assert_eq!(base_tool_briefs.len(), 1);
                assert_eq!(
                    base_tool_briefs[0].conversation_context_summary,
                    "用户在问上海天气"
                );
            }
            PlanParseOutcome::Invalid => panic!("expected ok"),
        }
    }

    #[test]
    fn one_retrieval_brief_per_channel_first_wins() {
        let raw = r#"{
          "original_query": "x",
          "briefs": [
            {"id":"t1","objective":"first rag","preferred_source":"rag","max_steps":3},
            {"id":"t2","objective":"second rag dropped","preferred_source":"rag","max_steps":2},
            {"id":"t3","objective":"web a","preferred_source":"web","queries":["a"],"max_steps":1},
            {"id":"t4","objective":"web b dropped","preferred_source":"web","queries":["b"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: true,
        };
        match parse_and_validate(raw, "x", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs, ..
            } => {
                assert_eq!(retrieval_briefs.len(), 2);
                assert_eq!(retrieval_briefs[0].sub_task.id, "t1");
                assert_eq!(
                    retrieval_briefs[0].sub_task.preferred_source,
                    PreferredSource::Rag
                );
                assert_eq!(retrieval_briefs[1].sub_task.id, "t3");
                assert_eq!(
                    retrieval_briefs[1].sub_task.preferred_source,
                    PreferredSource::Web
                );
            }
            PlanParseOutcome::Invalid => panic!("expected ok with two channel briefs"),
        }
    }

    #[test]
    fn duplicate_sub_task_id_dropped() {
        let raw = r#"{
          "original_query": "x",
          "briefs": [
            {"id":"same","objective":"first","preferred_source":"rag","max_steps":2},
            {"id":"same","objective":"dup","preferred_source":"web","queries":["q"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: true,
        };
        match parse_and_validate(raw, "x", caps) {
            PlanParseOutcome::Ok {
                retrieval_briefs, ..
            } => {
                assert_eq!(retrieval_briefs.len(), 1);
                assert_eq!(retrieval_briefs[0].sub_task.objective, "first");
            }
            PlanParseOutcome::Invalid => panic!("expected ok"),
        }
    }
}
