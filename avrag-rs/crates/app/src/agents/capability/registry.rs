use std::collections::HashMap;
use std::sync::OnceLock;

use super::{SkillMetadata, ToolMetadata};

static STANDARD_REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();

/// Global capability registry that unifies tools and skills under a single
/// query interface.
///
/// v5 architecture: all capabilities are registered here; strategies query
/// this registry at runtime to discover what tools and skills are available.
/// This replaces the v4 `ModeBundle` hard-coded tool lists.
pub struct CapabilityRegistry {
    tools: HashMap<String, ToolMetadata>,
    skills: HashMap<String, SkillMetadata>,
}

impl CapabilityRegistry {
    /// Build the standard registry from the existing compile-time registries.
    ///
    /// This bridges v4's `PromptRegistry` + `tool_catalog` into v5's
    /// `CapabilityRegistry`.  Tool/skill metadata is derived from the
    /// existing definitions with sensible defaults for v5-specific fields
    /// (risk_level, permissions, etc.).
    pub fn standard() -> Self {
        let mut tools = HashMap::new();
        let mut skills = HashMap::new();

        // --- Ingest tools from v4 tool catalogs ---
        for tool in super::super::progressive::rag_tool_catalog_cached() {
            let meta = tool_to_metadata(tool);
            tools.insert(meta.id.clone(), meta);
        }
        for tool in super::super::progressive::atomic_tool_catalog_cached() {
            let meta = tool_to_metadata(tool);
            tools.insert(meta.id.clone(), meta);
        }
        for tool in super::super::progressive::search_specific_tools_cached() {
            let meta = tool_to_metadata(tool);
            tools.insert(meta.id.clone(), meta);
        }

        // --- Ingest skills from v4 PromptRegistry ---
        let prompt_registry = super::super::progressive::PromptRegistry::standard_cached();
        for skill in prompt_registry.iter_skills() {
            let meta = skill_to_metadata(skill);
            skills.insert(meta.id.clone(), meta);
        }

        Self { tools, skills }
    }

    /// Lazily-initialised global singleton.
    pub fn standard_cached() -> &'static Self {
        STANDARD_REGISTRY.get_or_init(Self::standard)
    }

    /// Look up a tool by id.
    pub fn tool(&self, id: &str) -> Option<&ToolMetadata> {
        self.tools.get(id)
    }

    /// Look up a skill by id.
    pub fn skill(&self, id: &str) -> Option<&SkillMetadata> {
        self.skills.get(id)
    }

    /// List all registered tools.
    pub fn list_tools(&self) -> Vec<&ToolMetadata> {
        self.tools.values().collect()
    }

    /// List all registered skills.
    pub fn list_skills(&self) -> Vec<&SkillMetadata> {
        self.skills.values().collect()
    }

    /// Count of registered tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Count of registered skills.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers: convert v4 types into v5 metadata
// ---------------------------------------------------------------------------

fn tool_to_metadata(tool: &super::super::progressive::Tool) -> ToolMetadata {
    let spec = tool.spec();
    ToolMetadata {
        id: spec.name.clone(),
        version: spec.version.clone(),
        owner: "context-os".to_string(),
        description: spec.description.clone(),
        input_schema: spec.input_schema.clone(),
        output_schema: spec.output_schema.clone(),
        risk_level: infer_tool_risk_level(&spec.name),
        permissions: infer_tool_permissions(&spec.name),
        external_deps: infer_tool_external_deps(&spec.name),
        deprecation: None,
        retry_policy: infer_tool_retry_policy(&spec.name),
    }
}

fn skill_to_metadata(skill: &super::super::progressive::Skill) -> SkillMetadata {
    let md = skill.metadata();

    // Parse applicable_strategies from frontmatter if present
    let applicable_strategies = md
        .get("applicable_strategies")
        .map(|s| parse_string_list(s))
        .unwrap_or_else(|| infer_skill_strategies(skill.id()));

    // Parse required_tools from frontmatter if present
    let required_tools = md
        .get("required_tools")
        .map(|s| parse_string_list(s))
        .unwrap_or_default();

    // Parse risk_level from frontmatter if present
    let risk_level = md
        .get("risk_level")
        .and_then(|s| parse_risk_level(s))
        .unwrap_or_else(|| infer_skill_risk_level(skill.id()));

    SkillMetadata {
        id: skill.id().to_string(),
        version: skill.version().to_string(),
        owner: md
            .get("owner")
            .cloned()
            .unwrap_or_else(|| "context-os".to_string()),
        description: skill.description().to_string(),
        applicable_strategies,
        required_tools,
        risk_level,
        deprecation: None,
    }
}

// ---------------------------------------------------------------------------
// Inference helpers (temporary — will be replaced by explicit metadata
// in SKILL.md frontmatter and tool registration in Phase 1.3)
// ---------------------------------------------------------------------------

fn parse_string_list(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        inner
            .split(',')
            .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        vec![s.to_string()]
    }
}

fn parse_risk_level(s: &str) -> Option<super::RiskLevel> {
    match s.to_ascii_lowercase().as_str() {
        "low" => Some(super::RiskLevel::Low),
        "medium" => Some(super::RiskLevel::Medium),
        "high" => Some(super::RiskLevel::High),
        "critical" => Some(super::RiskLevel::Critical),
        _ => None,
    }
}

fn infer_tool_risk_level(name: &str) -> super::RiskLevel {
    match name {
        "web_search" => super::RiskLevel::High,       // external network
        "code_interpreter" => super::RiskLevel::High, // code execution
        "weather_query" => super::RiskLevel::Medium,  // external API
        _ => super::RiskLevel::Low,                    // internal retrieval
    }
}

fn infer_tool_permissions(name: &str) -> Vec<super::Permission> {
    match name {
        "web_search" => vec![super::Permission::ExternalNetwork],
        "code_interpreter" => vec![super::Permission::CodeExecution],
        _ => vec![super::Permission::User],
    }
}

fn infer_tool_external_deps(name: &str) -> Vec<String> {
    match name {
        "web_search" => vec!["search-provider".to_string()],
        "weather_query" => vec!["weather-api".to_string()],
        _ => Vec::new(),
    }
}

fn infer_tool_retry_policy(name: &str) -> super::RetryPolicy {
    match name {
        "web_search" => super::RetryPolicy {
            max_retries: 2,
            backoff_ms: 500,
            idempotent: true,
            ..super::RetryPolicy::default()
        },
        _ => super::RetryPolicy::default(),
    }
}

fn infer_skill_strategies(id: &str) -> Vec<String> {
    if id.starts_with("rag-") || id == "framework-extraction" {
        vec!["rag".to_string()]
    } else if id.starts_with("search-") {
        vec!["search".to_string()]
    } else if id.starts_with("chat") || id == "session-summary" {
        vec!["chat".to_string()]
    } else if ["ppt-generation", "html-renderer", "teaching"].contains(&id) {
        vec!["rag".to_string(), "chat".to_string()]
    } else {
        vec!["chat".to_string(), "rag".to_string(), "search".to_string()]
    }
}

fn infer_skill_risk_level(id: &str) -> super::RiskLevel {
    match id {
        "code_interpreter" | "web_search" => super::RiskLevel::High,
        _ => super::RiskLevel::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_loads_tools_and_skills() {
        let registry = CapabilityRegistry::standard();
        assert!(
            registry.tool_count() > 0,
            "registry should contain tools"
        );
        assert!(
            registry.skill_count() > 0,
            "registry should contain skills"
        );
    }

    #[test]
    fn standard_cached_returns_same_instance() {
        let r1 = CapabilityRegistry::standard_cached();
        let r2 = CapabilityRegistry::standard_cached();
        assert!(std::ptr::eq(r1, r2), "standard_cached should return the same instance");
    }

    #[test]
    fn can_lookup_rag_tools() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("dense_retrieval").is_some());
        assert!(registry.tool("lexical_retrieval").is_some());
        assert!(registry.tool("graph_retrieval").is_some());
    }

    #[test]
    fn can_lookup_atomic_tools() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("calculator").is_some());
        assert!(registry.tool("code_interpreter").is_some());
        assert!(registry.tool("weather_query").is_some());
    }

    #[test]
    fn can_lookup_search_tool() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("web_search").is_some());
    }

    #[test]
    fn can_lookup_skills() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.skill("rag-plan").is_some());
        assert!(registry.skill("rag-answer").is_some());
        assert!(registry.skill("chat").is_some());
        assert!(registry.skill("search-plan").is_some());
    }

    #[test]
    fn web_search_has_high_risk() {
        let registry = CapabilityRegistry::standard();
        let tool = registry.tool("web_search").unwrap();
        assert_eq!(tool.risk_level, super::super::RiskLevel::High);
        assert!(tool.permissions.contains(&super::super::Permission::ExternalNetwork));
    }

    #[test]
    fn rag_tools_are_low_risk() {
        let registry = CapabilityRegistry::standard();
        let tool = registry.tool("dense_retrieval").unwrap();
        assert_eq!(tool.risk_level, super::super::RiskLevel::Low);
    }

    #[test]
    fn list_tools_returns_all() {
        let registry = CapabilityRegistry::standard();
        let tools = registry.list_tools();
        assert!(
            tools.len() >= 11,
            "expected at least 11 tools (7 rag + 3 atomic + 1 search), got {}",
            tools.len()
        );
    }

    #[test]
    fn list_skills_returns_all() {
        let registry = CapabilityRegistry::standard();
        let skills = registry.list_skills();
        assert!(
            skills.len() >= 15,
            "expected at least 15 skills, got {}",
            skills.len()
        );
    }

    #[test]
    fn unknown_tool_returns_none() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("nonexistent").is_none());
    }

    #[test]
    fn unknown_skill_returns_none() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.skill("nonexistent").is_none());
    }

    #[test]
    fn rag_plan_reads_frontmatter_strategies() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("rag-plan").unwrap();
        assert_eq!(skill.applicable_strategies, vec!["rag"]);
    }

    #[test]
    fn rag_plan_reads_frontmatter_required_tools() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("rag-plan").unwrap();
        assert!(skill.required_tools.contains(&"dense_retrieval".to_string()));
        assert!(skill.required_tools.contains(&"lexical_retrieval".to_string()));
        assert_eq!(skill.required_tools.len(), 7);
    }

    #[test]
    fn chat_plan_reads_frontmatter_strategies() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("chat-plan").unwrap();
        assert_eq!(skill.applicable_strategies, vec!["chat"]);
    }

    #[test]
    fn search_plan_reads_frontmatter_strategies() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("search-plan").unwrap();
        assert_eq!(skill.applicable_strategies, vec!["search"]);
    }

    #[test]
    fn skills_without_frontmatter_fields_use_inference() {
        let registry = CapabilityRegistry::standard();
        // chat skill has no applicable_strategies in frontmatter — should infer from id
        let skill = registry.skill("chat").unwrap();
        assert!(skill.applicable_strategies.contains(&"chat".to_string()));
    }
}
