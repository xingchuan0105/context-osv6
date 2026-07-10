//! WriteRefine control-ring tool **schemas** (ADR-0007).
//!
//! Not registered on SkillRegistry / ToolCatalog. Disclosed only via
//! [`tool_specs_for_pool`] for `modes/write_refine.yaml` + WriteApp refine loop.
//! Runtime handlers live in write-core / app-chat writer (not SkillComponent::execute).

use contracts::ToolSpec;

const VERSION: &str = "1.0.0";
const REVISE_ID: &str = "write_refine_revise";
const RESEARCH_ID: &str = "write_refine_research";
const FINISH_ID: &str = "write_refine_finish";
const LEXICAL_ID: &str = "write_refine_lexical";

fn revise_spec() -> ToolSpec {
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

fn research_spec() -> ToolSpec {
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

fn finish_spec() -> ToolSpec {
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

fn lexical_spec() -> ToolSpec {
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

/// All write-control ToolSpecs (not registered on SkillRegistry).
pub fn all_tool_specs() -> Vec<ToolSpec> {
    vec![
        revise_spec(),
        research_spec(),
        finish_spec(),
        lexical_spec(),
    ]
}

/// Resolve ToolSpecs for a write_refine.yaml tool_pool (order preserved).
pub fn tool_specs_for_pool(pool: &[String]) -> Vec<ToolSpec> {
    let all = all_tool_specs();
    pool.iter()
        .filter_map(|id| all.iter().find(|s| s.name == *id).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revise_spec_has_required_fields() {
        let spec = revise_spec();
        assert_eq!(spec.name, REVISE_ID);
        assert_eq!(spec.version, VERSION);
        let schema = &spec.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("patches")));
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
        let spec = research_spec();
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
        assert!(kind["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("rag")));
    }

    #[test]
    fn finish_spec_requires_reason() {
        let spec = finish_spec();
        assert_eq!(spec.name, FINISH_ID);
        assert!(spec.input_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("reason")));
    }

    #[test]
    fn lexical_spec_has_ops() {
        let spec = lexical_spec();
        assert_eq!(spec.name, LEXICAL_ID);
        let op = &spec.input_schema["properties"]["op"];
        assert!(op["enum"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("repeat_term")));
    }

    #[test]
    fn tool_specs_for_pool_preserves_order() {
        let pool = vec![
            LEXICAL_ID.to_string(),
            REVISE_ID.to_string(),
            "unknown".to_string(),
        ];
        let specs = tool_specs_for_pool(&pool);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].name, LEXICAL_ID);
        assert_eq!(specs[1].name, REVISE_ID);
    }

    #[test]
    fn all_tool_specs_has_four() {
        assert_eq!(all_tool_specs().len(), 4);
    }
}
