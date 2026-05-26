//! Strategy prompt builders — v5 replacements for v4 ModeBundle prompt assembly.
//!
//! Each strategy calls these helpers with its own skill IDs.
//! Tool catalogs are queried from `CapabilityRegistry` by phase+strategy.
//! Skill bodies are loaded via `PromptRegistry` (v4 progressive layer).

use crate::agents::progressive::PromptRegistry;

// ---------------------------------------------------------------------------
// Plan-phase system prompt
// ---------------------------------------------------------------------------

/// Build the Plan-phase system prompt: planner skill body + tool catalog.
/// Tool catalog is queried from CapabilityRegistry by phase+strategy.
pub fn build_plan_system_prompt(
    planner_skill_id: &str,
    strategy: &str,
) -> String {
    let registry = PromptRegistry::standard_cached();
    let planner_body = registry
        .skill(planner_skill_id)
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    // 从 Registry 按 phase+strategy 查询工具目录（含 input_schema 参数）
    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_entries: Vec<String> = plan_tools
        .iter()
        .map(|t| {
            let header = format!("### {} (v{})\n{}", t.id, t.version, t.description);
            let params = format_tool_params(&t.input_schema);
            if params.is_empty() {
                header
            } else {
                format!("{header}\n{params}")
            }
        })
        .collect();
    let tool_catalog = tool_entries.join("\n\n");

    let mut parts = vec![planner_body];
    if !tool_catalog.is_empty() {
        parts.push(format!("## Available Tools\n\n{tool_catalog}"));
    }

    if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        parts.join("\n\n---\n\n")
    }
}

/// Build the Answer-phase system prompt: answer skill body + format skills catalog +
/// selected format skill bodies.
pub fn build_answer_system_prompt(
    answer_skill_id: &str,
    strategy: &str,
    selected_format_skills: &[String],
    selected_writing_styles: &[String],
) -> String {
    let registry = PromptRegistry::standard_cached();
    let mut parts = Vec::new();

    // 1. answer skill 全文（基底）
    if let Some(skill) = registry.skill(answer_skill_id) {
        parts.push(skill.system_prompt().to_string());
    }

    // 2. format 技能目录（Index tier）
    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let format_skills = cap_registry.answer_format_skills(strategy);
    if !format_skills.is_empty() {
        let catalog = format_skills
            .iter()
            .map(|s| format!("- {} (v{}): {}", s.id, s.version, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("## Available Output Formats\n\n{catalog}"));
    }

    // 3. 选中的 format skill 全文（Load tier）
    for skill_id in selected_format_skills {
        if let Some(skill) = registry.skill(skill_id.as_str()) {
            parts.push(skill.system_prompt().to_string());
        }
    }

    // 4. 写作风格目录（Index tier）
    let writing_styles = cap_registry.answer_writing_styles(strategy);
    if !writing_styles.is_empty() {
        let catalog = writing_styles
            .iter()
            .map(|s| format!("- {} (v{}): {}", s.id, s.version, s.description))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!("## Available Writing Styles\n\n{catalog}"));
    }

    // 5. 选中的 writing style 全文（Load tier）
    for skill_id in selected_writing_styles {
        if let Some(skill) = registry.skill(skill_id.as_str()) {
            parts.push(skill.system_prompt().to_string());
        }
    }

    parts.join("\n\n---\n\n")
}

/// Load a behavior mode skill body into the system prompt if active.
pub fn load_behavior_mode_skill(behavior_mode: Option<&str>) -> Option<String> {
    let mode = behavior_mode?;
    let registry = PromptRegistry::standard_cached();
    registry.skill(mode).map(|s| s.system_prompt().to_string())
}

/// Format tool input_schema properties as a human-readable parameter list.
fn format_tool_params(input_schema: &serde_json::Value) -> String {
    let Some(properties) = input_schema.get("properties").and_then(|p| p.as_object()) else {
        return String::new();
    };
    let required: Vec<&str> = input_schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut lines = vec!["Parameters:".to_string()];
    for (name, schema) in properties {
        let ty = schema.get("type").and_then(|t| t.as_str()).unwrap_or("any");
        let desc = schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        let req = if required.contains(&name.as_str()) {
            " (required)"
        } else {
            ""
        };
        let enum_vals = schema
            .get("enum")
            .and_then(|e| e.as_array())
            .map(|arr| {
                let vals: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| format!("\"{s}\"")))
                    .collect();
                format!(" [{}]", vals.join(", "))
            })
            .unwrap_or_default();
        lines.push(format!("  - {name}: {ty}{req}{enum_vals} — {desc}"));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Mode-specific constants
// ---------------------------------------------------------------------------

/// Skill and tool configuration for Chat mode.
pub mod chat {
    pub const PLANNER_SKILL_ID: &str = "chat-plan";
    pub const ANSWER_SKILL_ID: &str = "chat";
    pub const EVAL_SKILL_ID: Option<&str> = None;
}

/// Skill and tool configuration for RAG mode.
pub mod rag {
    pub const PLANNER_SKILL_ID: &str = "rag-plan";
    pub const EVAL_SKILL_ID: &str = "rag-eval";
    pub const ANSWER_SKILL_ID: &str = "rag-answer";
}

/// Skill and tool configuration for Search mode.
pub mod search {
    pub const PLANNER_SKILL_ID: &str = "search-plan";
    pub const EVAL_SKILL_ID: &str = "search-eval";
    pub const ANSWER_SKILL_ID: &str = "search-answer";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(chat::PLANNER_SKILL_ID, "chat");
        assert!(!prompt.is_empty());
        // Plan prompt should NOT contain format skills (those are Answer-phase only)
        assert!(!prompt.contains("Available Output Formats"));
    }

    #[test]
    fn rag_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(rag::PLANNER_SKILL_ID, "rag");
        assert!(!prompt.is_empty());
    }

    #[test]
    fn search_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(search::PLANNER_SKILL_ID, "search");
        assert!(!prompt.is_empty());
    }

    #[test]
    fn chat_answer_prompt_is_not_empty() {
        let prompt = build_answer_system_prompt(chat::ANSWER_SKILL_ID, "chat", &[], &[]);
        assert!(!prompt.is_empty());
        // Answer prompt should contain format skills catalog
        assert!(prompt.contains("Available Output Formats"));
    }
}
