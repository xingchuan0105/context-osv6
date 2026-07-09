/// Progressive disclosure assets (TN Wave 1 residual).
///
/// `PromptRegistry` is a **skill MD loader / disclosure catalog** only.
/// Tool **execution** is exclusively via [`crate::tool_registry`].
/// See `docs/agents/progressive-disclosure-framework.md` for architecture.
mod disclosure_unit;
mod prompt_registry;
mod skill;
mod skill_frontmatter;
mod tool;
mod tool_catalog;

pub use disclosure_unit::{DisclosureContext, DisclosureTier, DisclosureUnit};
pub use prompt_registry::PromptRegistry;
pub use skill::Skill;
pub use tool::Tool;
pub use tool_catalog::{
    atomic_tool_catalog, atomic_tool_catalog_cached, evaluate_calculator_expression,
    search_specific_tools, search_specific_tools_cached,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_render_includes_id_version_and_prompt() {
        let skill = Skill::new("test_skill", "A test skill.", "You are a test.");
        let ctx = DisclosureContext::with_tier(super::DisclosureTier::Load);
        let rendered = skill.render(&ctx);
        assert!(rendered.contains("test_skill"));
        assert!(rendered.contains("(v1.0)"));
        assert!(rendered.contains("You are a test."));
        // Perplexity principle: description is routing trigger, NOT included in render
        assert!(!rendered.contains("A test skill."));
    }

    #[test]
    fn tool_render_includes_name_version_description_and_schema() {
        let tool = Tool::new(contracts::ToolSpec {
            name: "test_tool".to_string(),
            version: "2.0".to_string(),
            description: "A test tool.".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: serde_json::json!({}),
        });
        let ctx = DisclosureContext::with_tier(super::DisclosureTier::Load);
        let rendered = tool.render(&ctx);
        assert!(rendered.contains("test_tool"), "missing name: {}", rendered);
        assert!(rendered.contains("2.0"), "missing version: {}", rendered);
        assert!(
            rendered.contains("A test tool."),
            "missing description: {}",
            rendered
        );
        assert!(
            rendered.contains("\"type\": \"object\""),
            "missing schema: {}",
            rendered
        );
    }

    #[test]
    fn prompt_registry_standard_cached_is_lazy() {
        let r1 = PromptRegistry::standard_cached();
        let r2 = PromptRegistry::standard_cached();
        assert!(
            std::ptr::eq(r1, r2),
            "standard_cached should return the same instance"
        );
    }

    #[test]
    fn prompt_registry_standard_cached_loads_cds_assets() {
        let registry = PromptRegistry::standard_cached();
        assert!(registry.skill("codegen").is_some());
        assert!(registry.skill("writing").is_some());
        assert!(registry.skill("format").is_some());
        assert!(registry.skill("memory").is_some());
        assert!(registry.skill("search").is_some());
        assert!(registry.skill("rag-answer").is_some());
        assert!(registry.skill("search-answer").is_some());
        assert!(registry.skill("chat").is_some());
        assert!(registry.skill("rag-system").is_some());
        assert!(registry.skill("dense-retrieval").is_none());
        assert!(registry.skill("doc-index").is_none());
        assert!(registry.skill("rag-plan").is_none());
        assert!(registry.skill("triplet-extraction").is_none());
        assert!(registry.skill("session-summary").is_none());
    }

    #[test]
    fn legacy_atomic_tool_catalog_is_empty_after_tool_specs_leave_prompts() {
        let tools = atomic_tool_catalog_cached();
        assert!(tools.is_empty());
    }

    #[test]
    fn atomic_tool_catalog_cached_is_lazy() {
        let c1 = atomic_tool_catalog_cached();
        let c2 = atomic_tool_catalog_cached();
        assert!(std::ptr::eq(c1.as_ptr(), c2.as_ptr()));
    }

    #[test]
    fn legacy_search_specific_tools_are_not_loaded_from_prompts() {
        let tools = search_specific_tools_cached();
        assert!(tools.is_empty());
    }

    #[test]
    fn search_specific_tools_cached_is_lazy() {
        let c1 = search_specific_tools_cached();
        let c2 = search_specific_tools_cached();
        assert!(std::ptr::eq(c1.as_ptr(), c2.as_ptr()));
    }
}
