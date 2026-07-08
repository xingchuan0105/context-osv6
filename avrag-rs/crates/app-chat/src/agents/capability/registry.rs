use std::collections::HashMap;
use std::sync::OnceLock;

use super::{ActivationPhase, Permission, RetryPolicy, SkillMetadata, ToolMetadata};
use crate::agents::skills::SkillComponent;

static STANDARD_REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();

/// Global capability registry for prompt skills and strategy metadata.
///
/// ADR-0007 D8: LLM-facing native tool schemas live here; modes disclose via `tool_pool`.
pub struct CapabilityRegistry {
    tools: HashMap<String, ToolMetadata>,
    skills: HashMap<String, SkillMetadata>,
    modes: HashMap<String, super::ModeSchema>,
}

impl CapabilityRegistry {
    /// Build the standard registry from prompt disclosure assets.
    pub fn standard() -> Self {
        let tools = register_llm_facing_tools();
        let mut skills = HashMap::new();

        // Prompt skills stay skills. Native tool schemas are disclosed via
        // ModeConfig::tool_pool, not inferred from SKILL.md metadata.
        let prompt_registry = super::super::progressive::PromptRegistry::standard_cached();
        for skill in prompt_registry.iter_skills() {
            let meta = skill_to_metadata(skill);
            if meta.deprecation.is_some() || is_retired_skill(&meta.id) {
                continue;
            }
            skills.insert(meta.id.clone(), meta);
        }

        // --- Static mode schemas (decoupled from strategy runtime) ---
        let mut modes = HashMap::new();
        for schema in super::schemas::standard_mode_schemas() {
            modes.insert(schema.id.clone(), schema);
        }

        Self {
            tools,
            skills,
            modes,
        }
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

    /// Look up a mode by id.
    pub fn mode(&self, id: &str) -> Option<&super::ModeSchema> {
        self.modes.get(id)
    }

    /// List all registered modes (按 ID 排序).
    pub fn list_modes(&self) -> Vec<&super::ModeSchema> {
        let mut modes: Vec<_> = self.modes.values().collect();
        modes.sort_by_key(|s| &s.id);
        modes
    }

    /// Count of registered modes.
    pub fn mode_count(&self) -> usize {
        self.modes.len()
    }

    /// Plan/Evaluate 阶段：返回指定模式可用的工具目录（按 ID 排序，确保 prompt 确定性）
    pub fn plan_tools(&self, mode_id: &str) -> Vec<&ToolMetadata> {
        let mode_id = mode_id.to_string();
        let mut tools: Vec<_> = self
            .tools
            .values()
            .filter(|t| t.activation_phase == ActivationPhase::PlanAndEvaluate)
            .filter(|t| t.applicable_strategies.iter().any(|s| s == &mode_id))
            .collect();
        tools.sort_by_key(|t| &t.id);
        tools
    }

    /// Answer 阶段：返回写作风格技能目录（按 ID 排序，确保 prompt 确定性）
    pub fn answer_writing_styles(&self, mode_id: &str) -> Vec<&SkillMetadata> {
        let mode_id = mode_id.to_string();
        let mut skills: Vec<_> = self
            .skills
            .values()
            .filter(|s| s.category == "writing-style")
            .filter(|s| s.applicable_strategies.iter().any(|s| s == &mode_id))
            .collect();
        skills.sort_by_key(|s| &s.id);
        skills
    }

}

// ---------------------------------------------------------------------------
// Helpers: convert v4 types into v5 metadata
// ---------------------------------------------------------------------------

/// ADR-0007 §8.8 retired skills — excluded from default registry catalog.
fn is_retired_skill(id: &str) -> bool {
    matches!(
        id,
        "rag-plan"
            | "search-plan"
            | "chat-plan"
            | "rag-eval"
            | "search-eval"
            | "rag-memory-mgmt"
            | "rag-citation-format"
            | "url-citation-format"
            | "rag-codegen-guide"
            | "rag-retrieval-strategy"
            | "rag-doc-summary-guide"
            | "concise-writing"
            | "professional-writing"
            | "academic-writing"
            | "storytelling"
            | "brainstorming"
            | "html-renderer"
            | "ppt-generation"
            | "teaching"
            | "framework-extraction"
    )
}

fn register_llm_facing_tools() -> HashMap<String, ToolMetadata> {
    use crate::agents::skills::builtin::{
        conversation_history::{ConversationHistoryLoad, UserProfileLoad},
        web_fetch::WebFetchSkill,
        web_search::WebSearchSkill,
    };

    let mut tools = HashMap::new();
    let all_modes = vec!["rag".to_string(), "search".to_string(), "chat".to_string()];
    let search = vec!["search".to_string()];
    let perms = vec![Permission::ExternalNetwork];

    insert_tool_from_skill(&mut tools, &WebSearchSkill, search.clone(), perms.clone());
    insert_tool_from_skill(&mut tools, &WebFetchSkill, search, perms);
    insert_tool_from_skill(
        &mut tools,
        &ConversationHistoryLoad,
        all_modes.clone(),
        vec![],
    );
    insert_tool_from_skill(&mut tools, &UserProfileLoad, all_modes, vec![]);
    tools
}

fn insert_tool_from_skill<S: SkillComponent>(
    tools: &mut HashMap<String, ToolMetadata>,
    skill: &S,
    applicable_strategies: Vec<String>,
    permissions: Vec<Permission>,
) {
    let spec = skill.spec();
    tools.insert(
        spec.name.clone(),
        tool_metadata_from_spec(&spec, applicable_strategies, permissions),
    );
}

fn tool_metadata_from_spec(
    spec: &contracts::ToolSpec,
    applicable_strategies: Vec<String>,
    permissions: Vec<Permission>,
) -> ToolMetadata {
    ToolMetadata {
        id: spec.name.clone(),
        version: spec.version.clone(),
        owner: "builtin".to_string(),
        description: spec.description.clone(),
        input_schema: spec.input_schema.clone(),
        output_schema: spec.output_schema.clone(),
        risk_level: infer_skill_risk_level(&spec.name),
        permissions,
        external_deps: vec![],
        deprecation: None,
        retry_policy: RetryPolicy::default(),
        activation_phase: ActivationPhase::PlanAndEvaluate,
        applicable_strategies,
    }
}

fn skill_to_metadata(skill: &super::super::progressive::Skill) -> SkillMetadata {
    let md = skill.metadata();

    // Parse applicable_strategies / applicable_modes from frontmatter if present
    let applicable_strategies = md
        .get("applicable_strategies")
        .or_else(|| md.get("applicable_modes"))
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

    // activation_phase: explicit frontmatter, else disclose_at (CDS clusters), else infer
    let activation_phase = md
        .get("activation_phase")
        .and_then(|s| parse_activation_phase(s))
        .or_else(|| {
            md.get("disclose_at").and_then(|s| match s.as_str() {
                "synthesis" => Some(ActivationPhase::Answer),
                "retrieve" => Some(ActivationPhase::PlanAndEvaluate),
                _ => None,
            })
        })
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
        deprecation: md.get("deprecation").map(|_| super::Deprecation {
            since_version: "adr-0007".to_string(),
            note: "Retired per ADR-0007".to_string(),
            replacement_id: None,
        }),
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
    match id {
        "codegen" => vec!["rag".to_string()],
        "search" => vec!["search".to_string()],
        id if id.starts_with("rag-") => vec!["rag".to_string()],
        id if id.starts_with("search-") => vec!["search".to_string()],
        id if id.starts_with("chat") => vec!["chat".to_string()],
        "format" | "writing" | "memory" => all(),
        _ => all(),
    }
}

fn infer_skill_risk_level(id: &str) -> super::RiskLevel {
    match id {
        "code_interpreter" | "web_search" | "web_fetch" => super::RiskLevel::High,
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
    match skill_id {
        "format"
        | "writing"
        | "html-renderer"
        | "ppt-generation"
        | "teaching"
        | "framework-extraction" => ActivationPhase::Answer,
        _ => ActivationPhase::PlanAndEvaluate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_loads_prompt_skills_and_search_tools() {
        let registry = CapabilityRegistry::standard();
        assert_eq!(registry.tool_count(), 4, "LLM-facing tools");
        assert!(registry.tool("web_search").is_some());
        assert!(registry.tool("web_fetch").is_some());
        assert!(registry.tool("conversation_history_load").is_some());
        assert!(registry.tool("user_profile_load").is_some());
        assert!(registry.skill_count() > 0, "registry should contain skills");
    }

    #[test]
    fn standard_cached_returns_same_instance() {
        let r1 = CapabilityRegistry::standard_cached();
        let r2 = CapabilityRegistry::standard_cached();
        assert!(
            std::ptr::eq(r1, r2),
            "standard_cached should return the same instance"
        );
    }

    #[test]
    fn rag_retrieval_tools_are_not_llm_facing_registry_tools() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("dense_retrieval").is_none());
        assert!(registry.tool("lexical_retrieval").is_none());
        assert!(registry.tool("graph_retrieval").is_none());
    }

    #[test]
    fn atomic_tools_are_not_registered_from_prompt_skills() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("calculator").is_none());
        assert!(registry.tool("code_interpreter").is_none());
        assert!(registry.tool("weather_query").is_none());
    }

    #[test]
    fn search_tool_schema_comes_from_capability_registry() {
        let registry = CapabilityRegistry::standard();
        let meta = registry.tool("web_search").expect("web_search in registry");
        assert!(meta.input_schema.get("properties").is_some());
        assert_eq!(meta.applicable_strategies, vec!["search"]);
    }

    #[test]
    fn can_lookup_skills() {
        let registry = CapabilityRegistry::standard();
        assert!(registry.skill("rag-answer").is_some());
        assert!(registry.skill("chat").is_some());
        assert!(registry.skill("rag-system").is_some());
    }

    #[test]
    fn retired_skills_excluded_from_standard_registry() {
        let registry = CapabilityRegistry::standard();
        for id in [
            "rag-plan",
            "search-plan",
            "chat-plan",
            "rag-eval",
            "search-eval",
            "rag-memory-mgmt",
            "rag-citation-format",
            "url-citation-format",
        ] {
            assert!(
                registry.skill(id).is_none(),
                "retired skill {id} should not be in standard registry"
            );
        }
    }

    #[test]
    fn search_mode_resolves_tool_pool_from_capability_registry() {
        let registry = CapabilityRegistry::standard();
        let mode = crate::agents::r#loop::config::load_mode_config("search")
            .expect("search mode config should load");
        let specs = mode.resolve_tool_specs(&registry, &mode.tool_pool);
        let names: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "web_search",
                "web_fetch",
                "conversation_history_load",
                "user_profile_load"
            ]
        );
        assert!(specs[0].input_schema.get("properties").is_some());
    }

    #[test]
    fn list_tools_returns_search_tools() {
        let registry = CapabilityRegistry::standard();
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 4);
    }

    #[test]
    fn list_skills_returns_all() {
        let registry = CapabilityRegistry::standard();
        let skills = registry.list_skills();
        assert!(
            skills.len() >= 10,
            "expected at least 10 prompt skills, got {}",
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
    fn rag_system_reads_frontmatter_strategies() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("rag-system").unwrap();
        assert_eq!(skill.applicable_strategies, vec!["rag"]);
    }

    #[test]
    fn codegen_cluster_is_registered() {
        let registry = CapabilityRegistry::standard();
        let skill = registry.skill("codegen").unwrap();
        assert_eq!(skill.applicable_strategies, vec!["rag"]);
    }

    #[test]
    fn skills_without_frontmatter_fields_use_inference() {
        let registry = CapabilityRegistry::standard();
        // chat skill has no applicable_strategies in frontmatter — should infer from id
        let skill = registry.skill("chat").unwrap();
        assert!(skill.applicable_strategies.contains(&"chat".to_string()));
    }

    #[test]
    fn registry_can_lookup_all_modes() {
        let registry = CapabilityRegistry::standard();
        assert_eq!(registry.mode_count(), 4, "expected 4 modes");
        assert!(registry.mode("chat").is_some());
        assert!(registry.mode("rag").is_some());
        assert!(registry.mode("search").is_some());
        assert!(registry.mode("write").is_some());
        assert!(registry.mode("nonexistent").is_none());
    }

    #[test]
    fn chat_mode_schema_has_expected_metadata() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.mode("chat").unwrap();
        assert_eq!(schema.id, "chat");
        assert!(!schema.requires_internet);
    }

    #[test]
    fn rag_mode_schema_has_expected_metadata() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.mode("rag").unwrap();
        assert_eq!(schema.id, "rag");
    }

    #[test]
    fn search_mode_schema_has_expected_metadata() {
        let registry = CapabilityRegistry::standard();
        let schema = registry.mode("search").unwrap();
        assert_eq!(schema.id, "search");
        assert!(schema.requires_internet);
    }

    #[test]
    fn list_modes_returns_all() {
        let registry = CapabilityRegistry::standard();
        let modes = registry.list_modes();
        assert_eq!(modes.len(), 4);
    }

    #[test]
    fn rag_plan_tools_include_memory_retrieval() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");
        let ids: Vec<&str> = plan_tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["conversation_history_load", "user_profile_load"]);
    }

    #[test]
    fn plan_tools_filters_by_phase() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");

        for tool in &plan_tools {
            assert_eq!(
                tool.activation_phase,
                super::super::ActivationPhase::PlanAndEvaluate
            );
        }

        assert_eq!(plan_tools.len(), 2);
    }
}
