/// Progressive Disclosure ReAct Loop Framework — unified agent loop with
/// per-phase disclosure of Tools and Skills.
///
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
    atomic_tool_catalog, atomic_tool_catalog_cached,
    evaluate_calculator_expression,
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
        let tool = Tool::new(common::ToolSpec {
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
        assert!(rendered.contains("A test tool."), "missing description: {}", rendered);
        assert!(rendered.contains("\"type\": \"object\""), "missing schema: {}", rendered);
    }

    #[test]
    fn prompt_registry_standard_cached_is_lazy() {
        let r1 = PromptRegistry::standard_cached();
        let r2 = PromptRegistry::standard_cached();
        assert!(std::ptr::eq(r1, r2), "standard_cached should return the same instance");
    }

    #[test]
    fn prompt_registry_standard_cached_loads_all_skills() {
        let registry = PromptRegistry::standard_cached();
        assert!(registry.skill("rag-plan").is_some());
        assert!(registry.skill("rag-answer").is_some());
        assert!(registry.skill("rag-eval").is_some());
        assert!(registry.skill("search-plan").is_some());
        assert!(registry.skill("search-answer").is_some());
        assert!(registry.skill("search-eval").is_some());
        assert!(registry.skill("chat-plan").is_some());
        assert!(registry.skill("chat").is_some());
    }

    #[test]
    fn atomic_tool_catalog_cached_has_all_atomic_tools() {
        let tools = atomic_tool_catalog_cached();
        assert!(tools.len() >= 3);
        let names: Vec<&str> = tools.iter().map(|t| t.spec().name.as_str()).collect();
        assert!(names.contains(&"calculator"));
        assert!(names.contains(&"code_interpreter"));
        assert!(names.contains(&"weather_query"));
    }

    #[test]
    fn atomic_tool_catalog_cached_is_lazy() {
        let c1 = atomic_tool_catalog_cached();
        let c2 = atomic_tool_catalog_cached();
        assert!(std::ptr::eq(c1.as_ptr(), c2.as_ptr()));
    }

    #[test]
    fn search_specific_tools_cached_has_web_search() {
        let tools = search_specific_tools_cached();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].spec().name, "web_search");
    }

    #[test]
    fn search_specific_tools_cached_is_lazy() {
        let c1 = search_specific_tools_cached();
        let c2 = search_specific_tools_cached();
        assert!(std::ptr::eq(c1.as_ptr(), c2.as_ptr()));
    }
}
