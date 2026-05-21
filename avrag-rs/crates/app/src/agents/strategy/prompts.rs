//! Strategy prompt builders — v5 replacements for v4 ModeBundle prompt assembly.
//!
//! Each strategy calls these helpers with its own skill IDs and tool lists.
//! Internally uses `PromptRegistry` and `DisclosureContext` (v4 progressive layer)
//! until skill body loading is fully migrated into `CapabilityRegistry`.

use crate::agents::progressive::{DisclosureContext, DisclosureTier, DisclosureUnit, PromptRegistry, Tool};

// ---------------------------------------------------------------------------
// Plan-phase system prompt
// ---------------------------------------------------------------------------

/// Build the Plan-phase system prompt: planner skill body + tool catalog +
/// optional format skills.
pub fn build_plan_system_prompt(
    planner_skill_id: &str,
    tools: &[Tool],
    format_skills: &[&str],
) -> String {
    let registry = PromptRegistry::standard_cached();
    let planner_body = registry
        .skill(planner_skill_id)
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    let tool_catalog = build_tool_catalog(tools);
    let format_skills_catalog = build_format_skills_catalog(format_skills);

    let mut parts = vec![planner_body];
    if !tool_catalog.is_empty() {
        parts.push(format!("## Available Tools\n\n{tool_catalog}"));
    }
    if !format_skills_catalog.is_empty() {
        parts.push(format!(
            "## Available Output Formats\n\n{format_skills_catalog}"
        ));
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
// Helpers
// ---------------------------------------------------------------------------

fn build_tool_catalog(tools: &[Tool]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    let ctx = DisclosureContext::with_tier(DisclosureTier::Index);
    let mut parts = Vec::new();
    for tool in tools {
        parts.push(tool.render(&ctx));
    }
    parts.join("\n---\n")
}

fn build_format_skills_catalog(skill_ids: &[&str]) -> String {
    if skill_ids.is_empty() {
        return String::new();
    }
    let registry = PromptRegistry::standard_cached();
    let mut lines = Vec::new();
    for id in skill_ids {
        if let Some(skill) = registry.skill(id) {
            lines.push(format!(
                "- {} (v{}): {}",
                skill.id(),
                skill.version(),
                skill.description()
            ));
        }
    }
    lines.join("\n")
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
        let prompt = build_plan_system_prompt(chat::PLANNER_SKILL_ID, &chat::plan_tools(), chat::format_skills());
        assert!(!prompt.is_empty());
    }

    #[test]
    fn rag_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(rag::PLANNER_SKILL_ID, &rag::plan_tools(), rag::format_skills());
        assert!(!prompt.is_empty());
    }

    #[test]
    fn search_plan_prompt_is_not_empty() {
        let prompt = build_plan_system_prompt(search::PLANNER_SKILL_ID, &search::plan_tools(), search::format_skills());
        assert!(!prompt.is_empty());
    }

    #[test]
    fn chat_answer_prompt_is_not_empty() {
        let prompt = build_answer_system_prompt(chat::ANSWER_SKILL_ID);
        assert!(!prompt.is_empty());
    }

    #[test]
    fn empty_tools_produces_empty_catalog() {
        let catalog = build_tool_catalog(&[]);
        assert!(catalog.is_empty());
    }

    #[test]
    fn empty_format_skills_produces_empty_catalog() {
        let catalog = build_format_skills_catalog(&[]);
        assert!(catalog.is_empty());
    }
}
