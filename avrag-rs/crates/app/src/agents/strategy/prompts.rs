//! Strategy prompt builders — v5 replacements for v4 ModeBundle prompt assembly.
//!
//! Each strategy calls these helpers with its own skill IDs.
//! Tool catalogs are queried from `CapabilityRegistry` by phase+strategy.
//! Skill bodies are loaded via `PromptRegistry` (v4 progressive layer).

use crate::agents::progressive::{PromptRegistry, Tool};

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

/// Build the Answer-phase system prompt from a skill ID.
pub fn build_answer_system_prompt(answer_skill_id: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    registry
        .skill(answer_skill_id)
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Mode-specific constants
// ---------------------------------------------------------------------------

/// Skill and tool configuration for Chat mode.
pub mod chat {
    use super::*;

    pub const PLANNER_SKILL_ID: &str = "chat-plan";
    pub const ANSWER_SKILL_ID: &str = "chat";
    pub const EVAL_SKILL_ID: Option<&str> = None;

    pub fn plan_tools() -> Vec<Tool> {
        let atomic = crate::agents::progressive::atomic_tool_catalog_cached();
        vec![
            find_tool(atomic, "calculator").expect("calculator must be in atomic catalog"),
            find_tool(atomic, "code_interpreter").expect("code_interpreter must be in atomic catalog"),
            find_tool(atomic, "weather_query").expect("weather_query must be in atomic catalog"),
        ]
    }

    pub fn format_skills() -> &'static [&'static str] {
        &[]
    }
}

/// Skill and tool configuration for RAG mode.
pub mod rag {
    use super::*;

    pub const PLANNER_SKILL_ID: &str = "rag-plan";
    pub const EVAL_SKILL_ID: &str = "rag-eval";
    pub const ANSWER_SKILL_ID: &str = "rag-answer";

    pub fn plan_tools() -> Vec<Tool> {
        crate::agents::progressive::rag_tool_catalog_cached().to_vec()
    }

    pub fn format_skills() -> &'static [&'static str] {
        &[
            "ppt-generation",
            "html-renderer",
            "teaching",
            "framework-extraction",
        ]
    }
}

/// Skill and tool configuration for Search mode.
pub mod search {
    use super::*;

    pub const PLANNER_SKILL_ID: &str = "search-plan";
    pub const EVAL_SKILL_ID: &str = "search-eval";
    pub const ANSWER_SKILL_ID: &str = "search-answer";

    pub fn plan_tools() -> Vec<Tool> {
        vec![]
    }

    pub fn format_skills() -> &'static [&'static str] {
        &[]
    }
}

fn find_tool(tools: &[Tool], name: &str) -> Option<Tool> {
    tools
        .iter()
        .find(|t| t.spec().name == name)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(chat::PLANNER_SKILL_ID, "chat");
        assert!(!prompt.is_empty());
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
        let prompt = build_answer_system_prompt(chat::ANSWER_SKILL_ID);
        assert!(!prompt.is_empty());
    }
}
