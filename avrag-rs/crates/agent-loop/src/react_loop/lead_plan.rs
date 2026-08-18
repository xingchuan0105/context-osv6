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
    /// Dual caps active and Lead chose retrieval, but briefs cover only one
    /// channel — an omitted channel is invisible to the host after dispatch
    /// (re-brief never invents one), so the whole plan is sent back for repair.
    DualChannelMissing,
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
    // Option (not just `default`): Lead models emit explicit `"base_tool": null`
    // for non-base briefs; `String` with only `default` fails the whole plan
    // parse on that null.
    #[serde(default)]
    base_tool: Option<String>,
    #[serde(default)]
    base_tool_arg: Option<String>,
    #[serde(default)]
    queries: Option<Vec<String>>,
    #[serde(default)]
    facets: Option<Vec<FacetWire>>,
    #[serde(default)]
    max_steps: Option<u8>,
    #[serde(default)]
    success_criteria: String,
    #[serde(default)]
    boundaries: String,
}

#[derive(Debug, Deserialize)]
struct FacetWire {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    objective: Option<String>,
}

impl FacetWire {
    /// Normalize to a brief-level facet; `None` when the objective is empty.
    fn into_facet(self, index: usize) -> Option<crate::lead_workers::Facet> {
        let objective = self.objective.unwrap_or_default();
        let objective = objective.trim();
        if objective.is_empty() {
            return None;
        }
        let id = self
            .id
            .unwrap_or_default()
            .trim()
            .to_string();
        let id = if id.is_empty() { format!("f{}", index + 1) } else { id };
        Some(crate::lead_workers::Facet { id, objective: objective.to_string() })
    }
}

/// Call Lead planner. On LLM failure or invalid JSON → `host_fallback` retrieval.
/// On valid plan with only base_tools/none → empty retrieval + optional base briefs.
pub async fn fetch_lead_briefs(
    llm: &LlmClient,
    request: &AgentRequest,
    caps: ActivatedCaps,
    plan_context_obs: &str,
    host_fallback: Vec<TaskBrief>,
    log: &mut super::run_log::RunEventLog,
) -> LeadPlanResult {
    use super::run_log::{PlanBriefSummary, RunEventKind};

    fn summaries(retrieval: &[TaskBrief], base_tools: &[TaskBrief]) -> Vec<PlanBriefSummary> {
        retrieval
            .iter()
            .chain(base_tools.iter())
            .map(|b| PlanBriefSummary {
                id: b.sub_task.id.clone(),
                source: b.sub_task.preferred_source.as_str().into(),
                objective: b.sub_task.objective.clone(),
            })
            .collect()
    }

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
        log.push(RunEventKind::PlanProposed {
            used_host_fallback: true,
            briefs: summaries(&host_fallback, &[]),
        });
        return LeadPlanResult {
            retrieval_briefs: host_fallback,
            base_tool_briefs: vec![],
            usage,
            used_host_fallback: true,
        };
    };
    usage.accumulate(&response.usage);
    let first_err = match parse_and_validate(&response.content, request.query.trim(), caps) {
        PlanParseOutcome::Ok {
            retrieval_briefs,
            base_tool_briefs,
        } => {
            log.push(RunEventKind::PlanProposed {
                used_host_fallback: false,
                briefs: summaries(&retrieval_briefs, &base_tool_briefs),
            });
            return LeadPlanResult {
                retrieval_briefs,
                base_tool_briefs,
                usage,
                used_host_fallback: false,
            };
        }
        PlanParseOutcome::Invalid => classify_parse_error(&response.content),
        PlanParseOutcome::DualChannelMissing => {
            "dual 双源激活且选择检索，但检索 brief 只覆盖了一侧通道；派工后被省略的通道对宿主不可见（无补派）".to_string()
        }
    };

    log.push(RunEventKind::PlanRepairRequested {
        reason: first_err.clone(),
        raw_preview: response.content.chars().take(300).collect(),
    });
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
        log.push(RunEventKind::PlanProposed {
            used_host_fallback: true,
            briefs: summaries(&host_fallback, &[]),
        });
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
        } => {
            log.push(RunEventKind::PlanProposed {
                used_host_fallback: false,
                briefs: summaries(&retrieval_briefs, &base_tool_briefs),
            });
            LeadPlanResult {
                retrieval_briefs,
                base_tool_briefs,
                usage,
                used_host_fallback: false,
            }
        }
        PlanParseOutcome::DualChannelMissing | PlanParseOutcome::Invalid => {
            log.push(RunEventKind::PlanProposed {
                used_host_fallback: true,
                briefs: summaries(&host_fallback, &[]),
            });
            LeadPlanResult {
                retrieval_briefs: host_fallback,
                base_tool_briefs: vec![],
                usage,
                used_host_fallback: true,
            }
        }
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
        // Facets: drop empty objectives, dedup ids (first wins), cap at
        // MAX_FACETS (normalization, not rejection — a fat plan still runs).
        let mut facet_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut facets: Vec<crate::lead_workers::Facet> = Vec::new();
        for (i, fw) in b.facets.unwrap_or_default().into_iter().enumerate() {
            let Some(f) = fw.into_facet(i) else { continue };
            if !facet_ids.insert(f.id.clone()) {
                continue;
            }
            facets.push(f);
            if facets.len() >= crate::lead_workers::MAX_FACETS {
                break;
            }
        }
        if source != PreferredSource::Rag {
            // facets 是 rag Worker 的顺序子检索机制；web 通道的多 query
            // 扇出由 `queries[]` 承载，base_tools/none 无检索。
            facets.clear();
        }
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
                base_tool: b.base_tool.unwrap_or_default(),
                base_tool_arg: b.base_tool_arg.unwrap_or_default(),
                facets,
                queries: b.queries.unwrap_or_default(),
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
    // Dual 双源：Lead 选择检索时两通道都必须有 brief。缺一侧在派工后对
    // 宿主不可见（re-brief 不补派被省略通道），整组打回重规划。
    // 空检索（base_tools/none）不受影响，走上面的短路径。
    if caps.rag && caps.search {
        let has_rag = retrieval
            .iter()
            .any(|b| b.sub_task.preferred_source == PreferredSource::Rag);
        let has_web = retrieval
            .iter()
            .any(|b| b.sub_task.preferred_source == PreferredSource::Web);
        if !has_rag || !has_web {
            return PlanParseOutcome::DualChannelMissing;
        }
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
            PlanParseOutcome::DualChannelMissing => panic!("unexpected dual miss"),
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
            PlanParseOutcome::DualChannelMissing => panic!("unexpected dual miss"),
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
            PlanParseOutcome::DualChannelMissing => panic!("BASE-only must not trigger dual gate"),
            PlanParseOutcome::Invalid => panic!("BASE-only plan must be Ok empty, not Invalid"),
        }
    }

    #[test]
    fn base_tools_brief_carries_llm_decided_tool_and_arg() {
        let raw = r#"{
          "original_query": "算一下 (1587+2933)*1.13",
          "briefs": [
            {"id":"t1","objective":"计算","preferred_source":"base_tools",
             "base_tool":"calculator","base_tool_arg":"(1587+2933)*1.13","max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: false,
            search: false,
        };
        match parse_and_validate(raw, "算一下 (1587+2933)*1.13", caps) {
            PlanParseOutcome::Ok {
                base_tool_briefs, ..
            } => {
                assert_eq!(base_tool_briefs.len(), 1);
                assert_eq!(base_tool_briefs[0].sub_task.base_tool, "calculator");
                assert_eq!(base_tool_briefs[0].sub_task.base_tool_arg, "(1587+2933)*1.13");
            }
            PlanParseOutcome::DualChannelMissing => panic!("unexpected dual miss"),
            PlanParseOutcome::Invalid => panic!("expected ok"),
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
            PlanParseOutcome::DualChannelMissing => panic!("unexpected dual miss"),
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
            PlanParseOutcome::DualChannelMissing => panic!("unexpected dual miss"),
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
            PlanParseOutcome::DualChannelMissing => panic!("rag+web both present, no dual miss"),
            PlanParseOutcome::Invalid => panic!("expected ok with two channel briefs"),
        }
    }

    #[test]
    fn duplicate_sub_task_id_dropped() {
        let raw = r#"{
          "original_query": "x",
          "briefs": [
            {"id":"same","objective":"first","preferred_source":"rag","max_steps":2},
            {"id":"same","objective":"dup","preferred_source":"base_tools","max_steps":1}
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
                assert_eq!(retrieval_briefs[0].sub_task.objective, "first");
            }
            PlanParseOutcome::DualChannelMissing => panic!("single-cap turn, no dual gate"),
            PlanParseOutcome::Invalid => panic!("expected ok"),
        }
    }

    #[test]
    fn dual_caps_web_only_plan_is_sent_back() {
        // q123/q124 型回归：dual 激活但 Lead 只派 web，rag 侧派工后不可见。
        let raw = r#"{
          "original_query": "文中 X 与公开资料有何差异",
          "briefs": [
            {"id":"t1","objective":"web 查公开资料","preferred_source":"web","queries":["x"],"max_steps":1}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: true,
        };
        assert_eq!(
            parse_and_validate(raw, "x", caps),
            PlanParseOutcome::DualChannelMissing
        );
    }

    #[test]
    fn facets_normalized_for_rag_and_dropped_for_web() {
        let raw = r#"{
          "original_query": "对比 A 和 B",
          "briefs": [
            {"id":"t1","objective":"对比检索","preferred_source":"rag","max_steps":3,
             "facets":[{"id":"fa","objective":"查 A"},{"objective":"查 B"},{"id":"fa","objective":"查 A 重复"},{"id":"fx","objective":"查 C"},{"id":"fy","objective":"查 D"},{"id":"fz","objective":"查 E 超限"}]},
            {"id":"t2","objective":"web 查","preferred_source":"web","queries":["x"],"max_steps":1,
             "facets":[{"id":"f1","objective":"web 不该有 facet"}]}
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
                let rag = &retrieval_briefs[0];
                let ids: Vec<&str> = rag.sub_task.facets.iter().map(|f| f.id.as_str()).collect();
                assert_eq!(ids, vec!["fa", "f2", "fx", "fy"], "dedup fa、补默认 id（按 wire 下标）、截断到 MAX_FACETS");
                // effective_facets 按 brief 前缀 scope
                let scoped: Vec<String> = rag
                    .sub_task
                    .effective_facets()
                    .into_iter()
                    .map(|f| f.id)
                    .collect();
                assert_eq!(scoped[0], "t1/fa");
                let web = &retrieval_briefs[1];
                assert!(web.sub_task.facets.is_empty(), "web brief 不承载 facets");
            }
            other => panic!("expected ok: {other:?}"),
        }
    }

    #[test]
    fn explicit_null_optional_fields_do_not_fail_plan() {        // qwen 习惯为非 base_tools brief 写 "base_tool": null / "queries": null；
        // explicit null 必须等价于缺省，不能毁掉整个 plan 解析。
        let raw = r#"{
          "original_query": "ADR-0004的决策日期",
          "briefs": [
            {"id":"t1","objective":"检索 ADR-0004","preferred_source":"rag","base_tool":null,"base_tool_arg":null,"queries":null,"max_steps":2}
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
                assert_eq!(retrieval_briefs[0].sub_task.base_tool, "");
                assert!(retrieval_briefs[0].sub_task.queries.is_empty());
            }
            other => panic!("explicit null must not fail plan parse: {other:?}"),
        }
    }

    #[test]
    fn dual_caps_rag_only_plan_is_sent_back() {        let raw = r#"{
          "original_query": "文中 X 与公开资料有何差异",
          "briefs": [
            {"id":"t1","objective":"kb 查文中表述","preferred_source":"rag","max_steps":2}
          ]
        }"#;
        let caps = ActivatedCaps {
            rag: true,
            search: true,
        };
        assert_eq!(
            parse_and_validate(raw, "x", caps),
            PlanParseOutcome::DualChannelMissing
        );
    }
}
