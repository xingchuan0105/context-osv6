use std::collections::HashMap;
use std::sync::OnceLock;

use super::{ActivationPhase, SkillMetadata, ToolMetadata};
use crate::catalog::ToolCatalog;

static STANDARD_REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();

/// Prompt-skill + mode metadata registry.
///
/// **Executable tools** live only in [`ToolCatalog`] (no second tool HashMap).
/// This type holds SKILL.md disclosure skills and static mode schemas; tool
/// lookups/list/count delegate to the unified catalog.
pub struct CapabilityRegistry {
    skills: HashMap<String, SkillMetadata>,
    modes: HashMap<String, super::ModeSchema>,
}

impl CapabilityRegistry {
    /// Build the standard registry from prompt disclosure assets.
    pub fn standard() -> Self {
        let mut skills = HashMap::new();

        // Prompt skills stay skills. Executable tool schemas come from ToolCatalog
        // (SkillComponent + RAG ids), disclosed via ModeConfig::tool_pool.
        let prompt_registry = crate::progressive::PromptRegistry::standard_cached();
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

        Self { skills, modes }
    }

    /// Lazily-initialised global singleton.
    pub fn standard_cached() -> &'static Self {
        STANDARD_REGISTRY.get_or_init(Self::standard)
    }

    /// Look up a tool by id (ReAct [`ToolCatalog`] only).
    ///
    /// Write-control tools (`write_refine_*`) are **not** here — see Write mode /
    /// `write_refine::tool_specs_for_pool`.
    pub fn tool(&self, id: &str) -> Option<&ToolMetadata> {
        let _ = self;
        ToolCatalog::standard_cached().tool_meta(id)
    }

    /// Look up a skill by id.
    pub fn skill(&self, id: &str) -> Option<&SkillMetadata> {
        self.skills.get(id)
    }

    /// All executable tool meta from [`ToolCatalog`] (execution table).
    ///
    /// **Not** the product disclosure list — HTTP capabilities use mode
    /// `tool_pool` (+ auto_fallback) via [`super::api::build_capabilities_response`].
    pub fn list_catalog_tools(&self) -> Vec<&ToolMetadata> {
        let _ = self;
        ToolCatalog::standard_cached()
            .list()
            .into_iter()
            .map(|t| &t.meta)
            .collect()
    }

    /// List all registered skills.
    pub fn list_skills(&self) -> Vec<&SkillMetadata> {
        self.skills.values().collect()
    }

    /// Count of executable tools (catalog).
    pub fn tool_count(&self) -> usize {
        let _ = self;
        ToolCatalog::standard_cached().len()
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
        let _ = self;
        let mode_id = mode_id.to_string();
        let mut tools: Vec<_> = ToolCatalog::standard_cached()
            .list()
            .into_iter()
            .map(|t| &t.meta)
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
///
/// Sorted alphabetically to enable `binary_search`; edit this list to retire
/// or revive a skill. A skill may also self-declare via the `deprecation`
/// frontmatter key (handled separately in `skill_to_metadata`).
const RETIRED_SKILL_IDS: &[&str] = &[
    "academic-writing",
    "brainstorming",
    "chat-plan",
    "concise-writing",
    "framework-extraction",
    "html-renderer",
    "ppt-generation",
    "professional-writing",
    "rag-codegen-guide",
    "rag-citation-format",
    "rag-doc-summary-guide",
    "rag-eval",
    "rag-memory-mgmt",
    "rag-plan",
    "rag-retrieval-strategy",
    "search-eval",
    "search-plan",
    "storytelling",
    "teaching",
    "url-citation-format",
];

fn is_retired_skill(id: &str) -> bool {
    RETIRED_SKILL_IDS.binary_search(&id).is_ok()
}

fn skill_to_metadata(skill: &crate::progressive::Skill) -> SkillMetadata {
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
    fn standard_registry_loads_prompt_skills_and_catalog_tools() {
        let registry = CapabilityRegistry::standard();
        // Tools come solely from ToolCatalog (skills + RAG), not a second map.
        // write_refine_* excluded from ReAct ToolCatalog (Write control ring).
        assert!(
            registry.tool_count() >= 6 + crate::catalog::RAG_TOOL_IDS.len(),
            "catalog tools (non-write builtins + RAG)"
        );
        assert!(
            registry.tool("write_refine_revise").is_none(),
            "write_refine must not appear on CapabilityRegistry/ToolCatalog"
        );
        assert!(registry.tool("web_search").is_some());
        assert!(registry.tool("web_fetch").is_some());
        assert!(registry.tool("conversation_history_load").is_some());
        assert!(registry.tool("user_profile_load").is_some());
        assert!(registry.tool("calculator").is_some());
        assert!(registry.tool("dense_retrieval").is_some());
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
    fn tool_lookup_projects_unified_catalog_for_rag_and_builtins() {
        // TN: CapabilityRegistry.tool falls through to ToolCatalog so all
        // executable tools are visible for policy / mode resolution.
        let registry = CapabilityRegistry::standard();
        assert!(registry.tool("dense_retrieval").is_some());
        assert!(registry.tool("lexical_retrieval").is_some());
        assert!(registry.tool("calculator").is_some());
        assert!(registry.tool("code_interpreter").is_some());
        // Still not registered as prompt *skills* (disclosure SKILL.md layer).
        assert!(registry.skill("calculator").is_none());
        assert!(registry.skill("dense_retrieval").is_none());
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
    fn search_mode_tool_pool_resolves_from_capability_registry() {
        // Mode YAML lives under app-chat loop loader; assert registry has the
        // tools that search.yaml declares without depending on mode_loader.
        let registry = CapabilityRegistry::standard();
        let tool_pool = [
            "web_search",
            "web_fetch",
            "conversation_history_load",
            "user_profile_load",
        ];
        let specs: Vec<_> = tool_pool
            .iter()
            .filter_map(|id| registry.tool(id))
            .collect();
        assert_eq!(specs.len(), tool_pool.len());
        assert!(specs[0].input_schema.get("properties").is_some());
    }

    #[test]
    fn list_catalog_tools_returns_unified_catalog() {
        let registry = CapabilityRegistry::standard();
        let tools = registry.list_catalog_tools();
        assert_eq!(tools.len(), registry.tool_count());
        assert!(tools.iter().any(|t| t.id == "web_search"));
        assert!(tools.iter().any(|t| t.id == "dense_retrieval"));
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
    fn rag_plan_tools_include_memory_and_retrieval() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");
        let ids: Vec<&str> = plan_tools.iter().map(|t| t.id.as_str()).collect();
        // Unified catalog: memory tools + RAG channel tools (+ other rag-applicable builtins).
        assert!(ids.contains(&"conversation_history_load"));
        assert!(ids.contains(&"user_profile_load"));
        assert!(ids.contains(&"dense_retrieval"));
        assert!(!ids.contains(&"web_search"), "search-only tools excluded from rag plan");
    }

    #[test]
    fn plan_tools_filters_by_phase() {
        let registry = CapabilityRegistry::standard();
        let plan_tools = registry.plan_tools("rag");

        for tool in &plan_tools {
            assert_eq!(
                tool.activation_phase,
                crate::capability::ActivationPhase::PlanAndEvaluate
            );
            assert!(
                tool.applicable_strategies.iter().any(|s| s == "rag"),
                "{} should apply to rag",
                tool.id
            );
        }

        assert!(plan_tools.len() >= 2);
    }
}
