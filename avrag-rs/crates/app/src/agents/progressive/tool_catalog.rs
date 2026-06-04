use std::sync::OnceLock;

use super::Tool;

static ATOMIC_TOOL_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();
static SEARCH_SPECIFIC_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();

// ============================================================================
// Atomic tools (all modes)
// ============================================================================

/// Build the universal atomic tool catalog shared across all agent modes.
///
/// These tools are disclosed in the Plan and Execute phases regardless of
/// the agent mode (Chat / RAG / Search).
///
/// Definitions are loaded from the `SkillRegistry` so adding a new atomic
/// tool only requires registering a `SkillComponent` — no edits here.
///
/// For hot paths prefer [`atomic_tool_catalog_cached`].
pub fn atomic_tool_catalog() -> Vec<Tool> {
    let registry = crate::agents::skills::registry::builtin_registry_cached();
    registry
        .iter()
        .map(|skill| {
            let gotchas = skill.gotchas().iter().map(|s| s.to_string()).collect();
            Tool::new(skill.spec()).with_gotchas(gotchas)
        })
        .collect()
}

/// Return a lazily-initialised global singleton of the atomic tool catalog.
pub fn atomic_tool_catalog_cached() -> &'static [Tool] {
    ATOMIC_TOOL_CATALOG.get_or_init(atomic_tool_catalog).as_slice()
}

// ============================================================================
// Calculator evaluation helper
// ============================================================================

/// Evaluate a mathematical expression string and return the numeric result.
///
/// Delegates to the calculator skill implementation so the logic lives in
/// one place (`skills/builtin/calculator.rs`).
pub fn evaluate_calculator_expression(expression: &str) -> Result<f64, String> {
    crate::agents::skills::builtin::calculator::evaluate_calculator_expression(expression)
}

// ============================================================================
// Search-specific tools
// ============================================================================

/// Build the search-specific tool catalog.
///
/// These tools are disclosed only when the agent mode is Search.
/// Loaded from the `SkillRegistry` so the definition lives in one place.
///
/// For hot paths prefer [`search_specific_tools_cached`].
pub fn search_specific_tools() -> Vec<Tool> {
    match crate::agents::skills::registry::builtin_registry_cached().get("web_search") {
        Some(skill) => {
            let gotchas = skill.gotchas().iter().map(|s| s.to_string()).collect();
            vec![Tool::new(skill.spec()).with_gotchas(gotchas)]
        }
        None => vec![],
    }
}

/// Return a lazily-initialised global singleton of the search-specific tool catalog.
pub fn search_specific_tools_cached() -> &'static [Tool] {
    SEARCH_SPECIFIC_CATALOG.get_or_init(search_specific_tools).as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_tool_catalog_has_all_atomic_tools() {
        let tools = atomic_tool_catalog();
        assert!(tools.len() >= 3);
        let names: Vec<&str> = tools.iter().map(|t| t.spec().name.as_str()).collect();
        assert!(names.contains(&"calculator"));
        assert!(names.contains(&"code_interpreter"));
        assert!(names.contains(&"weather_query"));
    }
}
