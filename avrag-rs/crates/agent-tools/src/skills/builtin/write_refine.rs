//! WriteRefine loop native tools — `write_refine_revise` / `write_refine_research` /
//! `write_refine_finish`.
//!
//! These are LLM-facing native tools registered in `CapabilityRegistry` and
//! disclosed to the WriteRefine ReAct loop via `modes/write_refine.yaml::tool_pool`.
//!
//! **Runtime routing:** the real handlers live in `WriteRefineLoopRunner`
//! (`crate::writer::refine_loop`), which intercepts these three tool ids and
//! dispatches to `handle_revise` / `handle_research` / `handle_finish` *before*
//! they ever reach `SkillComponent::execute`. The `execute` bodies below are
//! therefore defensive stubs that return `ToolStatus::NotImplemented` — they
//! exist only to satisfy the `SkillComponent` trait and keep the registry
//! self-consistent. The schema/gotchas here are the source of truth for what
//! the LLM sees.

use contracts::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::skills::{ExecutionContext, SkillComponent};

/// `write_refine_revise` — apply sentence-level patches to the numbered draft.
pub struct WriteRefineReviseSkill;

/// `write_refine_research` — fetch additional grounding material during refinement.
pub struct WriteRefineResearchSkill;

/// `write_refine_finish` — end refinement (soft finish) and return the best draft.
pub struct WriteRefineFinishSkill;

/// `write_refine_lexical` — repeat or replace terms for hapax/zipf steering.
pub struct WriteRefineLexicalSkill;

const REVISE_ID: &str = "write_refine_revise";
const RESEARCH_ID: &str = "write_refine_research";
const FINISH_ID: &str = "write_refine_finish";
const LEXICAL_ID: &str = "write_refine_lexical";
const VERSION: &str = "1";

fn not_implemented_result(id: &str, version: &str) -> ToolResult {
    ToolResult {
        tool: id.to_string(),
        version: version.to_string(),
        status: ToolStatus::NotImplemented,
        data: Some(serde_json::json!({
            "error": "write_refine tools are dispatched by WriteRefineLoopRunner before reaching execute(); this stub is unreachable in production"
        })),
        trace: None,
    }
}

#[async_trait::async_trait]
impl SkillComponent for WriteRefineReviseSkill {
    fn id(&self) -> &str {
        REVISE_ID
    }

    fn version(&self) -> &str {
        VERSION
    }

    fn description(&self) -> &str {
        "Load when refining a numbered draft and the agent decides to rewrite one or more sentences."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: REVISE_ID.to_string(),
            version: VERSION.to_string(),
            description: concat!(
                "Apply sentence-level patches to the numbered draft.\n",
                "Rules:\n",
                "- Each patch targets one live sentence id (`s<n>`).\n",
                "- `text` must end with 。！？ and be at least 2 characters.\n",
                "- Up to 12 patches per call; failed patches return a tool error and do not count against the revise round budget."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["patches"],
                "properties": {
                    "patches": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 12,
                        "items": {
                            "type": "object",
                            "required": ["id", "text"],
                            "properties": {
                                "id": { "type": "string", "pattern": "^s[0-9]+$" },
                                "text": { "type": "string", "minLength": 2 }
                            }
                        }
                    },
                    "note": { "type": "string", "maxLength": 200 }
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "applied": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "ok": { "type": "boolean" },
                                "error": { "type": "string" }
                            }
                        }
                    },
                    "diagnosis_delta": { "type": "object" },
                    "revise_rounds_used": { "type": "integer" }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "id must be a live sentence in DraftWorkspace; stale ids from prior rounds are rejected.",
            "text must end with 。！？ — mid-sentence fragments are not patches.",
            "A failed patch does not consume a revise round; retry with a corrected id/text.",
        ]
    }

    fn render_hint(&self) -> &str {
        "json"
    }

    async fn execute<'a>(&self, _args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        not_implemented_result(REVISE_ID, VERSION)
    }
}

#[async_trait::async_trait]
impl SkillComponent for WriteRefineResearchSkill {
    fn id(&self) -> &str {
        RESEARCH_ID
    }

    fn version(&self) -> &str {
        VERSION
    }

    fn description(&self) -> &str {
        "Load when refining a draft and missing facts or grounding material not in the appendix."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: RESEARCH_ID.to_string(),
            version: VERSION.to_string(),
            description: concat!(
                "Fetch additional grounding material during refinement.\n",
                "Rules:\n",
                "- `kind=rag` dispatches a RAG sub-worker; `kind=web` dispatches a Search sub-worker.\n",
                "- Global cap: 5 calls per refine loop (separate from initial-draft research budget).\n",
                "- The 6th call returns `budget_exhausted`."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["kind", "query"],
                "properties": {
                    "kind": { "type": "string", "enum": ["rag", "web"] },
                    "query": { "type": "string", "minLength": 4, "maxLength": 500 },
                    "reason": { "type": "string", "maxLength": 200 }
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "new_cards": {
                        "type": "array",
                        "maxItems": 3,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "kind": { "type": "string" },
                                "content": { "type": "string" },
                                "source_label": { "type": "string" },
                                "rare_terms": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "terms": { "type": "array", "items": { "type": "string" } },
                    "research_calls_used": { "type": "integer" },
                    "budget_exhausted": { "type": "boolean" }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "Hard cap is 5 calls per refine loop, tracked separately from the initial draft research budget.",
            "Sub-workers run with reduced budgets: max_iterations=2, per_research_worker_tokens=4000.",
            "Observation returns a compressed summary (≤3 new cards + term list), never full text.",
        ]
    }

    fn render_hint(&self) -> &str {
        "json"
    }

    async fn execute<'a>(&self, _args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        not_implemented_result(RESEARCH_ID, VERSION)
    }
}

#[async_trait::async_trait]
impl SkillComponent for WriteRefineFinishSkill {
    fn id(&self) -> &str {
        FINISH_ID
    }

    fn version(&self) -> &str {
        VERSION
    }

    fn description(&self) -> &str {
        "Load when the refined draft is good enough to end the refinement loop (soft finish)."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: FINISH_ID.to_string(),
            version: VERSION.to_string(),
            description: concat!(
                "End refinement and return the best draft version.\n",
                "Rules:\n",
                "- Calling finish is a soft exit: the orchestrator takes the best_version and runs validate.\n",
                "- If bands are not fully satisfied, the result carries validation_warning but the draft is still delivered.\n",
                "- `bands_satisfied` is telemetry-only and does not act as a hard gate."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["reason"],
                "properties": {
                    "reason": { "type": "string", "minLength": 4, "maxLength": 500 },
                    "bands_satisfied": { "type": "boolean" }
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "finish_reason": { "type": "string" },
                    "bands_satisfied": { "type": "boolean" },
                    "validation_warning": { "type": "boolean" }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "finish is a soft exit — bands_satisfied is telemetry only, not a gate.",
            "Hard exits (iteration/token/round caps) also produce a soft finish via best-version, with a degrade trace entry.",
        ]
    }

    fn render_hint(&self) -> &str {
        "json"
    }

    async fn execute<'a>(&self, _args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        not_implemented_result(FINISH_ID, VERSION)
    }
}

#[async_trait::async_trait]
impl SkillComponent for WriteRefineLexicalSkill {
    fn id(&self) -> &str {
        LEXICAL_ID
    }

    fn version(&self) -> &str {
        VERSION
    }

    fn description(&self) -> &str {
        "Load when diagnosis shows hapax or zipf band failures and vocabulary-level repeat/replace is needed."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: LEXICAL_ID.to_string(),
            version: VERSION.to_string(),
            description: concat!(
                "Apply vocabulary-level edits without rewriting whole sentences.\n",
                "Ops:\n",
                "- `repeat_term`: weave `term` into sentences missing it (lowers hapax).\n",
                "- `replace_term`: replace `from` with `to` up to `max_replacements` (can raise zipf).\n",
                "Failed ops return tool error; successful edits count as one revise round."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["op"],
                "properties": {
                    "op": { "type": "string", "enum": ["repeat_term", "replace_term"] },
                    "term": { "type": "string", "minLength": 2 },
                    "from": { "type": "string", "minLength": 1 },
                    "to": { "type": "string", "minLength": 1 },
                    "sentence_ids": {
                        "type": "array",
                        "items": { "type": "string", "pattern": "^s[0-9]+$" },
                        "maxItems": 12
                    },
                    "max_edits": { "type": "integer", "minimum": 1, "maximum": 12 },
                    "max_replacements": { "type": "integer", "minimum": 1, "maximum": 12 },
                    "note": { "type": "string", "maxLength": 200 }
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "edits": { "type": "array" },
                    "diagnosis_delta": { "type": "object" },
                    "revise_rounds_used": { "type": "integer" }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "repeat_term replaces one content word per target sentence; term must not already appear there.",
            "replace_term changes at most one occurrence per sentence per call.",
            "When hapax/zipf bands fail, write_refine_finish may be rejected until they pass.",
        ]
    }

    fn render_hint(&self) -> &str {
        "json"
    }

    async fn execute<'a>(&self, _args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        not_implemented_result(LEXICAL_ID, VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revise_spec_has_required_fields() {
        let spec = WriteRefineReviseSkill.spec();
        assert_eq!(spec.name, REVISE_ID);
        assert_eq!(spec.version, VERSION);
        let schema = &spec.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::json!("patches")));
        let patches = &schema["properties"]["patches"];
        assert_eq!(patches["type"], "array");
        assert_eq!(patches["minItems"], 1);
        assert_eq!(patches["maxItems"], 12);
        let item = &patches["items"];
        assert_eq!(item["properties"]["id"]["pattern"], "^s[0-9]+$");
        assert_eq!(item["properties"]["text"]["minLength"], 2);
    }

    #[test]
    fn research_spec_has_kind_and_query() {
        let spec = WriteRefineResearchSkill.spec();
        assert_eq!(spec.name, RESEARCH_ID);
        let schema = &spec.input_schema;
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(required.contains(&"kind"));
        assert!(required.contains(&"query"));
        let kind = &schema["properties"]["kind"];
        assert!(kind["enum"].as_array().unwrap().contains(&serde_json::json!("rag")));
        assert!(kind["enum"].as_array().unwrap().contains(&serde_json::json!("web")));
    }

    #[test]
    fn finish_spec_requires_reason() {
        let spec = WriteRefineFinishSkill.spec();
        assert_eq!(spec.name, FINISH_ID);
        let schema = &spec.input_schema;
        assert_eq!(schema["required"], serde_json::json!(["reason"]));
        assert_eq!(schema["properties"]["reason"]["minLength"], 4);
        assert_eq!(schema["properties"]["reason"]["maxLength"], 500);
    }

    #[tokio::test]
    async fn revise_execute_returns_not_implemented() {
        let ctx = ExecutionContext::new(None);
        let result = WriteRefineReviseSkill
            .execute(&serde_json::json!({}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }

    #[tokio::test]
    async fn research_execute_returns_not_implemented() {
        let ctx = ExecutionContext::new(None);
        let result = WriteRefineResearchSkill
            .execute(&serde_json::json!({}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }

    #[tokio::test]
    async fn finish_execute_returns_not_implemented() {
        let ctx = ExecutionContext::new(None);
        let result = WriteRefineFinishSkill
            .execute(&serde_json::json!({}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }
}