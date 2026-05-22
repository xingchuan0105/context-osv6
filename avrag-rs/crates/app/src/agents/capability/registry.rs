use std::collections::HashMap;
use std::sync::OnceLock;

use super::{ActivationPhase, SkillMetadata, ToolMetadata};
use crate::agents::strategy::Strategy;

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
    strategies: HashMap<String, super::StrategySchema>,
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

        // --- Ingest strategy schemas from v5 Strategy implementations ---
        let mut strategies = HashMap::new();
        let chat_schema = crate::agents::strategy::chat::ChatStrategy::schema();
        strategies.insert(chat_schema.id.clone(), chat_schema);
        let rag_schema = crate::agents::strategy::rag::RagStrategy::schema();
        strategies.insert(rag_schema.id.clone(), rag_schema);
        let search_schema = crate::agents::strategy::search::SearchStrategy::schema();
        strategies.insert(search_schema.id.clone(), search_schema);
        let composite_schema = crate::agents::strategy::composite::CompositeStrategy::schema();
        strategies.insert(composite_schema.id.clone(), composite_schema);

        Self { tools, skills, strategies }
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

    /// Look up a strategy by id.
    pub fn strategy(&self, id: &str) -> Option<&super::StrategySchema> {
        self.strategies.get(id)
    }

    /// List all registered strategies.
    pub fn list_strategies(&self) -> Vec<&super::StrategySchema> {
        self.strategies.values().collect()
    }

    /// Count of registered strategies.
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
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
        activation_phase: ActivationPhase::PlanAndEvaluate,
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

    // 新增：从 frontmatter 解析 activation_phase
    let activation_phase = md
        .get("activation_phase")
        .and_then(|s| parse_activation_phase(s))
        .unwrap_or_else(|| infer_skill_activation_phase(skill.id()));

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
        activation_phase,
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

fn parse_activation_phase(s: &str) -> Option<ActivationPhase> {
    match s.to_lowercase().as_str() {
        "plan_and_evaluate" | "planandevalue" => Some(ActivationPhase::PlanAndEvaluate),
        "answer" => Some(ActivationPhase::Answer),
        _ => None,
    }
}

fn infer_skill_activation_phase(skill_id: &str) -> ActivationPhase {
    // format 技能默认 Answer，其他技能默认 PlanAndEvaluate
    if skill_id == "html-renderer"
        || skill_id == "ppt-generation"
        || skill_id == "teaching"
        || skill_id == "framework-extraction"
    {
        ActivationPhase::Answer
    } else {
        ActivationPhase::PlanAndEvaluate
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

    #[test]
    fn registry_can_lookup_all_strategies() {
        let registry = CapabilityRegistry::standard();
        assert_eq!(registry.strategy_count(), 4, "expected 4 strategies");
        assert!(registry.strategy("chat").is_some());
        assert!(registry.strategy("rag").is_some());
        assert!(registry.strategy("search").is_some());
        assert!(registry.strategy("composite").is_some());
        assert!(registry.strategy("nonexistent").is_none());
    }

    #[test]
    fn chat_strategy_schema_matches_state_machine() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.strategy("chat").unwrap();
        assert_eq!(schema.id, "chat");
        assert_eq!(schema.states, vec!["Plan", "ExecuteAtomic", "Answer"]);
        assert_eq!(schema.max_budget, 1);
        assert!(!schema.requires_internet);
    }

    #[test]
    fn rag_strategy_schema_has_replan_loop() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.strategy("rag").unwrap();
        assert_eq!(schema.id, "rag");
        assert_eq!(schema.states, vec!["Plan", "ExecuteRetrieve", "Evaluate", "Answer"]);
        assert_eq!(schema.max_budget, 4);
        let has_replan = schema.transitions.iter().any(|t| t.from == "Evaluate" && t.to == "Plan");
        assert!(has_replan, "RAG strategy should have Evaluate→Plan replan transition");
    }

    #[test]
    fn search_strategy_schema_has_replan_loop() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.strategy("search").unwrap();
        assert_eq!(schema.id, "search");
        assert_eq!(schema.states, vec!["Decompose", "ParallelSearch", "Aggregate", "Evaluate", "Answer"]);
        assert_eq!(schema.max_budget, 3);
        assert!(schema.requires_internet);
        let has_replan = schema.transitions.iter().any(|t| t.from == "Evaluate" && t.to == "ParallelSearch");
        assert!(has_replan, "Search strategy should have Evaluate→ParallelSearch replan transition");
    }

    #[test]
    fn composite_strategy_schema_matches_state_machine() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.strategy("composite").unwrap();
        assert_eq!(schema.id, "composite");
        assert_eq!(schema.states, vec!["Decompose", "ParallelExecute", "Merge", "Answer"]);
        assert_eq!(schema.max_budget, 4);
        assert!(schema.requires_internet);
    }

    #[test]
    fn list_strategies_returns_all() {
        let registry = CapabilityRegistry::standard();
        let strategies = registry.list_strategies();
        assert_eq!(strategies.len(), 4);
    }
}
