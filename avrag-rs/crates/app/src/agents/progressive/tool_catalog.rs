use std::sync::OnceLock;

use super::Tool;

static ATOMIC_TOOL_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();
static SEARCH_SPECIFIC_CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();

// ============================================================================
// Atomic tools (all modes)
// ============================================================================

/// Build the universal atomic tool catalog from declarative SKILL.md.
///
/// Reads skills with `category: atomic-tool` from PromptRegistry and
/// converts them to `Tool` instances for v4 compatibility.
///
/// For hot paths prefer [`atomic_tool_catalog_cached`].
pub fn atomic_tool_catalog() -> Vec<Tool> {
    let prompt_registry = crate::agents::progressive::PromptRegistry::standard_cached();
    prompt_registry
        .iter_skills()
        .filter(|s| s.metadata().get("category") == Some(&"atomic-tool".to_string()))
        .map(|skill| {
            let gotchas = skill
                .references()
                .get("gotchas.md")
                .map(|g| {
                    g.lines()
                        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
                        .map(|l| {
                            l.trim_start_matches("- ")
                                .trim_start_matches("* ")
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let input_schema = skill
                .input_schema()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({"type": "object"}));

            let output_schema = skill
                .output_schema()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));

            Tool::new(common::ToolSpec {
                name: skill.id().replace('-', "_"),
                version: skill.version().to_string(),
                description: skill.description().to_string(),
                input_schema,
                output_schema,
            })
            .with_gotchas(gotchas)
        })
        .collect()
}

/// Return a lazily-initialised global singleton of the atomic tool catalog.
pub fn atomic_tool_catalog_cached() -> &'static [Tool] {
    ATOMIC_TOOL_CATALOG
        .get_or_init(atomic_tool_catalog)
        .as_slice()
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

/// Build the search-specific tool catalog from declarative SKILL.md.
///
/// Reads the `web_search` skill from PromptRegistry.
/// For hot paths prefer [`search_specific_tools_cached`].
pub fn search_specific_tools() -> Vec<Tool> {
    let prompt_registry = crate::agents::progressive::PromptRegistry::standard_cached();
    prompt_registry
        .skill("web_search")
        .map(|skill| {
            let gotchas = skill
                .references()
                .get("gotchas.md")
                .map(|g| {
                    g.lines()
                        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
                        .map(|l| {
                            l.trim_start_matches("- ")
                                .trim_start_matches("* ")
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let input_schema = skill
                .input_schema()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({"type": "object"}));

            let output_schema = skill
                .output_schema()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));

            vec![
                Tool::new(common::ToolSpec {
                    name: skill.id().replace('-', "_"),
                    version: skill.version().to_string(),
                    description: skill.description().to_string(),
                    input_schema,
                    output_schema,
                })
                .with_gotchas(gotchas),
            ]
        })
        .unwrap_or_default()
}

/// Return a lazily-initialised global singleton of the search-specific tool catalog.
pub fn search_specific_tools_cached() -> &'static [Tool] {
    SEARCH_SPECIFIC_CATALOG
        .get_or_init(search_specific_tools)
        .as_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_tool_catalog_has_all_atomic_tools() {
        let tools = atomic_tool_catalog();
        assert!(
            tools.len() >= 3,
            "expected at least 3 atomic tools, got {}",
            tools.len()
        );
        let names: Vec<&str> = tools.iter().map(|t| t.spec().name.as_str()).collect();
        assert!(names.contains(&"calculator"), "missing calculator");
        assert!(
            names.contains(&"code_interpreter"),
            "missing code_interpreter"
        );
        assert!(names.contains(&"weather_query"), "missing weather_query");
    }
}
