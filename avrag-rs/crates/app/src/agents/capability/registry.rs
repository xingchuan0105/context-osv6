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

        // NOTE: v4 hard-coded tool catalogs (rag_tool_catalog, atomic_tool_catalog,
        // search_specific_tools) have been migrated to declarative SKILL.md.
        // All tool metadata now flows from PromptRegistry → skills map →
        // dual-flavor registration (skills + tools) above.

        // --- Ingest skills from v4 PromptRegistry ---
        let prompt_registry = super::super::progressive::PromptRegistry::standard_cached();
        for skill in prompt_registry.iter_skills() {
            let meta = skill_to_metadata(skill);
            skills.insert(meta.id.clone(), meta.clone());

            // NEW: skills that carry an input_schema are also registered as
            // tools so that plan_tools(strategy) can discover them alongside
            // legacy v4 hard-coded tools.  The tool id is the skill id with
            // kebab-case converted to snake_case so it matches the runtime
            // dispatch names (e.g. dense-retrieval → dense_retrieval).
            if meta.input_schema.is_some() {
                let tool_id = meta.id.replace('-', "_");
                let permissions = match tool_id.as_str() {
                    "web_search" => vec![super::Permission::ExternalNetwork],
                    "code_interpreter" => vec![super::Permission::CodeExecution],
                    _ => Vec::new(),
                };
                let tool_meta = ToolMetadata {
                    id: tool_id,
                    version: meta.version.clone(),
                    owner: meta.owner.clone(),
                    description: meta.description.clone(),
                    input_schema: meta.input_schema.clone().unwrap_or(serde_json::Value::Null),
                    output_schema: meta.output_schema.clone().unwrap_or(serde_json::Value::Null),
                    risk_level: meta.risk_level,
                    permissions,
                    external_deps: Vec::new(),
                    deprecation: meta.deprecation.clone(),
                    retry_policy: super::RetryPolicy::default(),
                    activation_phase: meta.activation_phase,
                    applicable_strategies: meta.applicable_strategies.clone(),
                };
                tools.insert(tool_meta.id.clone(), tool_meta);
            }
        }

        // --- Ingest strategy schemas from v5 Strategy implementations ---
        let mut strategies = HashMap::new();
        let chat_schema = crate::agents::strategy::chat::ChatStrategy::schema();
        strategies.insert(chat_schema.id.clone(), chat_schema);
        let rag_schema = crate::agents::strategy::rag::RagStrategy::schema();
        strategies.insert(rag_schema.id.clone(), rag_schema);
        let search_schema = crate::agents::strategy::search::SearchStrategy::schema();
        strategies.insert(search_schema.id.clone(), search_schema);

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

    /// List all registered strategies (按 ID 排序).
    pub fn list_strategies(&self) -> Vec<&super::StrategySchema> {
        let mut strategies: Vec<_> = self.strategies.values().collect();
        strategies.sort_by_key(|s| &s.id);
        strategies
    }

    /// Count of registered strategies.
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    /// Plan/Evaluate 阶段：返回指定策略可用的工具目录（按 ID 排序，确保 prompt 确定性）
    pub fn plan_tools(&self, strategy: &str) -> Vec<&ToolMetadata> {
        let strategy = strategy.to_string();
        let mut tools: Vec<_> = self
            .tools
            .values()
            .filter(|t| t.activation_phase == ActivationPhase::PlanAndEvaluate)
            .filter(|t| t.applicable_strategies.iter().any(|s| s == &strategy))
            .collect();
        tools.sort_by_key(|t| &t.id);
        tools
    }

    /// Answer 阶段：返回 format 技能目录（按 ID 排序，确保 prompt 确定性）
    pub fn answer_format_skills(&self, strategy: &str) -> Vec<&SkillMetadata> {
        let strategy = strategy.to_string();
        let mut skills: Vec<_> = self
            .skills
            .values()
            .filter(|s| s.activation_phase == ActivationPhase::Answer)
            .filter(|s| s.applicable_strategies.iter().any(|s| s == &strategy))
            .collect();
        skills.sort_by_key(|s| &s.id);
        skills
    }

    /// Answer 阶段：返回写作风格技能目录（按 ID 排序，确保 prompt 确定性）
    pub fn answer_writing_styles(&self, strategy: &str) -> Vec<&SkillMetadata> {
        let strategy = strategy.to_string();
        let mut skills: Vec<_> = self
            .skills
            .values()
            .filter(|s| s.category == "writing-style")
            .filter(|s| s.applicable_strategies.iter().any(|s| s == &strategy))
            .collect();
        skills.sort_by_key(|s| &s.id);
        skills
    }

    /// Answer 阶段：返回行为模式技能目录（目前只有 brainstorming，按 ID 排序）
    pub fn answer_behavior_modes(&self, strategy: &str) -> Vec<&SkillMetadata> {
        let strategy = strategy.to_string();
        let mut skills: Vec<_> = self
            .skills
            .values()
            .filter(|s| s.category == "behavior")
            .filter(|s| s.applicable_strategies.iter().any(|s| s == &strategy))
            .collect();
        skills.sort_by_key(|s| &s.id);
        skills
    }
}

// ---------------------------------------------------------------------------
// Helpers: convert v4 types into v5 metadata
// ---------------------------------------------------------------------------

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

    // Parse category from frontmatter if present
    let category = md
        .get("category")
        .cloned()
        .unwrap_or_else(|| "standard".to_string());

    // Parse JSON schemas from the skill's declarative schema files.
    let input_schema = skill
        .input_schema()
        .and_then(|s| serde_json::from_str(s).ok());
    let output_schema = skill
        .output_schema()
        .and_then(|s| serde_json::from_str(s).ok());

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
        category,
        input_schema,
        output_schema,
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

fn infer_skill_strategies(id: &str) -> Vec<String> {
    let all = || vec!["chat".to_string(), "rag".to_string(), "search".to_string()];
    if id.starts_with("rag-") {
        vec!["rag".to_string()]
    } else if id.starts_with("search-") {
        vec!["search".to_string()]
    } else if id.starts_with("chat") || id == "session-summary" {
        vec!["chat".to_string()]
    } else if ["ppt-generation", "html-renderer", "teaching", "framework-extraction"].contains(&id) {
        // Format skills are output-agnostic — available to all strategies
        all()
    } else {
        all()
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
        assert_eq!(registry.strategy_count(), 3, "expected 3 strategies");
        assert!(registry.strategy("chat").is_some());
        assert!(registry.strategy("rag").is_some());
        assert!(registry.strategy("search").is_some());
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
        let has_re_execute = schema.transitions.iter().any(|t| t.from == "Evaluate" && t.to == "ExecuteRetrieve");
        assert!(has_re_execute, "RAG strategy should have Evaluate→ExecuteRetrieve transition for direct tool re-calls");
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
    fn list_strategies_returns_all() {
        let registry = CapabilityRegistry::standard();
        let strategies = registry.list_strategies();
        assert_eq!(strategies.len(), 3);
    }

    #[test]
    fn plan_tools_includes_skills_with_input_schema() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");
        // 7 atomic tool skills (with input_schema) should appear as tools
        assert!(plan_tools.iter().any(|t| t.id == "dense_retrieval"), "dense_retrieval from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "lexical_retrieval"), "lexical_retrieval from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "graph_retrieval"), "graph_retrieval from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "doc_index"), "doc_index from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "index_lookup"), "index_lookup from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "doc_summary"), "doc_summary from skill should be in plan_tools");
        assert!(plan_tools.iter().any(|t| t.id == "doc_metadata"), "doc_metadata from skill should be in plan_tools");
    }

    #[test]
    fn plan_tools_filters_by_phase() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");

        // 所有返回的工具都应该是 PlanAndEvaluate phase
        for tool in &plan_tools {
            assert_eq!(tool.activation_phase, super::super::ActivationPhase::PlanAndEvaluate);
        }

        // 应该包含 RAG 工具
        assert!(plan_tools.iter().any(|t| t.id == "dense_retrieval"));
    }

    #[test]
    fn answer_format_skills_filters_by_phase() {
        let registry = CapabilityRegistry::standard();
        let answer_skills = registry.answer_format_skills("rag");

        // 所有返回的技能都应该是 Answer phase
        for skill in &answer_skills {
            assert_eq!(skill.activation_phase, super::super::ActivationPhase::Answer);
        }

        // 应该包含 format 技能
        assert!(answer_skills.iter().any(|s| s.id == "html-renderer"));
        assert!(answer_skills.iter().any(|s| s.id == "ppt-generation"));
    }

    #[test]
    fn answer_format_skills_universal_across_strategies() {
        let registry = CapabilityRegistry::standard();

        let format_ids = ["ppt-generation", "html-renderer", "teaching", "framework-extraction"];

        // Format skills are output-agnostic — available to all strategies
        for strategy in ["chat", "rag", "search"] {
            let skills = registry.answer_format_skills(strategy);
            for id in &format_ids {
                assert!(
                    skills.iter().any(|s| s.id == *id),
                    "format skill '{id}' should be available to strategy '{strategy}'"
                );
            }
        }
    }
}
