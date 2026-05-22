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

    // 从 Registry 按 phase+strategy 查询工具目录
    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools
        .iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

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

    parts.join("\n\n---\n\n")
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
        let prompt = build_answer_system_prompt(chat::ANSWER_SKILL_ID, "chat", &[]);
        assert!(!prompt.is_empty());
        // Answer prompt should contain format skills catalog
        assert!(prompt.contains("Available Output Formats"));
    }
}
