use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeConfig {
    #[serde(alias = "mode")]
    pub id: String,
    pub system_prompt_base: String,
    /// Tool ids disclosed to the LLM during retrieve. Schemas resolved from
    /// [`CapabilityRegistry`](crate::agents::capability::CapabilityRegistry).
    #[serde(default)]
    pub tool_pool: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_skill_catalog")]
    pub skill_catalog: SkillCatalogConfig,
    /// Inject retrieval/display query block during retrieve (and synthesis when true).
    #[serde(default)]
    pub inject_retrieval_query: bool,
    pub budget: BudgetConfig,
    pub auto_fallback: Option<AutoFallbackConfig>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub query_normalization: QueryNormalizationConfig,
    #[serde(default)]
    pub loop_exit: LoopExitConfig,
    #[serde(default)]
    pub synthesis_output: SynthesisOutputConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryNormalizationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_prior_turns")]
    pub max_prior_turns: u8,
    #[serde(default = "default_true")]
    pub llm_fallback: bool,
}

impl Default for QueryNormalizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_prior_turns: 6,
            llm_fallback: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoopExitConfig {
    #[serde(default)]
    pub require_evidence: bool,
    #[serde(default)]
    pub allow_content_early_stop: bool,
    #[serde(default)]
    pub skip_synthesis_on_direct_answer: bool,
    #[serde(default)]
    pub evidence_gate: Option<EvidenceGateConfig>,
}

impl Default for LoopExitConfig {
    fn default() -> Self {
        Self {
            require_evidence: true,
            allow_content_early_stop: false,
            skip_synthesis_on_direct_answer: false,
            evidence_gate: None,
        }
    }
}

/// Pure-code evidence quality gate configuration.
/// No LLM calls — inspects retrieval metadata only.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvidenceGateConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_min_top_score")]
    pub min_top_score: f32,
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
    #[serde(default = "default_true")]
    pub topic_overlap_required: bool,
}

impl Default for EvidenceGateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_top_score: 0.5,
            max_context_tokens: 12000,
            topic_overlap_required: true,
        }
    }
}

fn default_min_top_score() -> f32 {
    0.5
}

fn default_max_context_tokens() -> usize {
    12000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerContractKind {
    InternalAnswerV1,
    InternalSearchAnswerV1,
    ProseOnly,
}

impl Default for AnswerContractKind {
    fn default() -> Self {
        Self::InternalAnswerV1
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SynthesisOutputConfig {
    #[serde(default)]
    pub contract: AnswerContractKind,
}

impl Default for SynthesisOutputConfig {
    fn default() -> Self {
        Self {
            contract: AnswerContractKind::InternalAnswerV1,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_prior_turns() -> u8 {
    6
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SkillCatalogConfig {
    #[serde(default)]
    pub retrieve_clusters: Vec<String>,
    #[serde(default)]
    pub synthesis_clusters: Vec<String>,
    #[serde(default)]
    pub clusters: Vec<SkillCluster>,
    #[serde(default)]
    pub mandatory: MandatorySkills,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillCluster {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    pub skills: Vec<String>,
    #[serde(default)]
    pub atomic: bool,
    #[serde(default)]
    pub disclose_at: DiscloseAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscloseAt {
    #[default]
    Retrieve,
    Synthesis,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MandatorySkills {
    /// Clusters forced on the first retrieve round (e.g. RAG `codegen`).
    #[serde(default)]
    pub retrieve: Vec<String>,
    #[serde(default, alias = "mandatory_synthesis")]
    pub synthesis: Vec<String>,
}

impl SkillCatalogConfig {
    pub fn clusters_at(&self, phase: DiscloseAt) -> Vec<&SkillCluster> {
        self.clusters
            .iter()
            .filter(|c| c.disclose_at == phase)
            .collect()
    }

    pub fn cluster_by_id(&self, id: &str) -> Option<&SkillCluster> {
        self.clusters.iter().find(|c| c.id == id)
    }

    pub fn flat_skill_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for cluster in &self.clusters {
            for skill in &cluster.skills {
                if !ids.contains(skill) {
                    ids.push(skill.clone());
                }
            }
        }
        for skill in &self.mandatory.synthesis {
            if !ids.contains(skill) {
                ids.push(skill.clone());
            }
        }
        ids
    }

    pub fn expand_cluster_skills(&self, cluster_id: &str) -> Vec<String> {
        if self.cluster_by_id(cluster_id).is_some() {
            return vec![cluster_id.to_string()];
        }
        Vec::new()
    }

    /// Build cluster entries from CDS v1.1 yaml lists + PromptRegistry metadata.
    pub fn hydrate_clusters(&mut self) {
        if !self.clusters.is_empty()
            && self.retrieve_clusters.is_empty()
            && self.synthesis_clusters.is_empty()
        {
            return;
        }
        if self.retrieve_clusters.is_empty() && self.synthesis_clusters.is_empty() {
            return;
        }

        let registry = super::super::progressive::PromptRegistry::standard_cached();
        let mut clusters = Vec::new();

        for id in &self.retrieve_clusters {
            if let Some(cluster) = cluster_from_registry(registry, id, DiscloseAt::Retrieve) {
                clusters.push(cluster);
            }
        }
        for id in &self.synthesis_clusters {
            if let Some(cluster) = cluster_from_registry(registry, id, DiscloseAt::Synthesis) {
                clusters.push(cluster);
            }
        }
        self.clusters = clusters;
    }
}

fn cluster_from_registry(
    registry: &super::super::progressive::PromptRegistry,
    id: &str,
    default_phase: DiscloseAt,
) -> Option<SkillCluster> {
    let skill = registry.skill(id)?;
    let md = skill.metadata();
    let disclose_at = md
        .get("disclose_at")
        .and_then(|v| match v.as_str() {
            "retrieve" => Some(DiscloseAt::Retrieve),
            "synthesis" => Some(DiscloseAt::Synthesis),
            _ => None,
        })
        .unwrap_or(default_phase);
    let atomic = md.get("atomic").map(|v| v == "true").unwrap_or(false);
    Some(SkillCluster {
        id: id.to_string(),
        description: Some(skill.description().to_string()),
        skills: vec![id.to_string()],
        atomic,
        disclose_at,
    })
}

fn deserialize_skill_catalog<'de, D>(deserializer: D) -> Result<SkillCatalogConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};

    struct SkillCatalogVisitor;
    impl<'de> Visitor<'de> for SkillCatalogVisitor {
        type Value = SkillCatalogConfig;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a skill catalog list or structured object")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut legacy_ids = Vec::new();
            while let Some(id) = seq.next_element::<String>()? {
                legacy_ids.push(id);
            }
            Ok(SkillCatalogConfig {
                clusters: legacy_ids
                    .into_iter()
                    .map(|id| SkillCluster {
                        id: id.clone(),
                        description: None,
                        skills: vec![id],
                        atomic: false,
                        disclose_at: DiscloseAt::Retrieve,
                    })
                    .collect(),
                ..Default::default()
            })
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            let mut retrieve_clusters = Vec::new();
            let mut synthesis_clusters = Vec::new();
            let mut clusters = Vec::new();
            let mut mandatory = MandatorySkills::default();
            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "retrieve" | "retrieve_clusters" => {
                        retrieve_clusters = map.next_value()?;
                    }
                    "synthesis" | "synthesis_clusters" => {
                        synthesis_clusters = map.next_value()?;
                    }
                    "clusters" => {
                        clusters = map.next_value()?;
                    }
                    "mandatory" => {
                        mandatory = map.next_value()?;
                    }
                    "mandatory_retrieve" => {
                        mandatory.retrieve = map.next_value()?;
                    }
                    "mandatory_synthesis" => {
                        mandatory.synthesis = map.next_value()?;
                    }
                    _ => {
                        let _: de::IgnoredAny = map.next_value()?;
                    }
                }
            }
            Ok(SkillCatalogConfig {
                retrieve_clusters,
                synthesis_clusters,
                clusters,
                mandatory,
            })
        }
    }

    deserializer.deserialize_any(SkillCatalogVisitor)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetConfig {
    pub max_iterations: u8,
    #[serde(default)]
    pub by_user_tier: Option<HashMap<String, u8>>,
}

impl BudgetConfig {
    pub fn resolve_max_iterations(&self, request_tier: Option<&serde_json::Value>) -> u8 {
        let tier_str = request_tier
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let resolved = if let Some(tier) = tier_str {
            self.by_user_tier
                .as_ref()
                .and_then(|m| m.get(&tier).copied())
                .unwrap_or(self.max_iterations)
        } else {
            self.max_iterations
        };
        resolved.max(1)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoFallbackConfig {
    pub enabled: bool,
    pub tool_id: String,
    pub top_k: u8,
    #[serde(default)]
    pub vertical: Option<String>,
}

pub fn load_mode_config(mode_id: &str) -> Result<ModeConfig, common::AppError> {
    let mut resolved_path = std::path::PathBuf::from(format!("modes/{}.yaml", mode_id));
    if !resolved_path.exists() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_path = std::path::PathBuf::from(manifest_dir)
                .join("../..")
                .join(format!("modes/{}.yaml", mode_id));
            if workspace_path.exists() {
                resolved_path = workspace_path;
            }
        }
    }
    if !resolved_path.exists() {
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let check_path = dir.join("modes").join(format!("{}.yaml", mode_id));
            if check_path.exists() {
                resolved_path = check_path;
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let content = std::fs::read_to_string(&resolved_path).map_err(|e| {
        common::AppError::internal(format!(
            "failed to read mode config at {:?}: {}",
            resolved_path, e
        ))
    })?;
    let mut config: ModeConfig = serde_yaml::from_str(&content)
        .map_err(|e| common::AppError::internal(format!("failed to parse mode config: {e}")))?;
    config.normalize();
    config.validate()?;
    Ok(config)
}

impl ModeConfig {
    pub fn loop_exit_for_mode(&self) -> LoopExitConfig {
        let mut cfg = self.loop_exit.clone();
        if self.id == "chat" {
            if !self.loop_exit.require_evidence
                && !self.loop_exit.allow_content_early_stop
                && !self.loop_exit.skip_synthesis_on_direct_answer
            {
                cfg.require_evidence = false;
                cfg.allow_content_early_stop = true;
                cfg.skip_synthesis_on_direct_answer = true;
            }
        } else if (self.id == "rag" || self.id == "search")
            && !self.loop_exit.require_evidence
            && self.loop_exit.allow_content_early_stop
        {
            cfg.require_evidence = true;
            cfg.allow_content_early_stop = false;
            cfg.skip_synthesis_on_direct_answer = false;
        }
        // Enable evidence gate by default for RAG and search modes
        if (self.id == "rag" || self.id == "search") && cfg.evidence_gate.is_none() {
            cfg.evidence_gate = Some(EvidenceGateConfig::default());
        }
        cfg
    }

    pub fn normalize(&mut self) {
        self.skill_catalog.hydrate_clusters();
    }

    pub fn validate(&self) -> Result<(), common::AppError> {
        if self.id.is_empty() {
            return Err(common::AppError::validation(
                "mode_config",
                "mode id is empty",
            ));
        }
        if self.budget.max_iterations == 0 {
            return Err(common::AppError::validation(
                "mode_config",
                "budget.max_iterations must be > 0",
            ));
        }
        Ok(())
    }

    pub fn mandatory_synthesis_skills(&self) -> &[String] {
        &self.skill_catalog.mandatory.synthesis
    }

    pub fn resolve_tool_specs(
        &self,
        registry: &crate::agents::capability::CapabilityRegistry,
        ids: &[String],
    ) -> Vec<common::ToolSpec> {
        ids.iter()
            .filter_map(|id| registry.tool(id).map(tool_metadata_to_spec))
            .collect()
    }

    /// Resolve tool specs for the retrieve phase from `tool_pool`.
    pub fn tools_for_retrieve(
        &self,
        registry: &crate::agents::capability::CapabilityRegistry,
    ) -> Vec<common::ToolSpec> {
        if self.tool_pool.is_empty() {
            return vec![];
        }
        self.resolve_tool_specs(registry, &self.tool_pool)
    }
}

fn tool_metadata_to_spec(meta: &crate::agents::capability::ToolMetadata) -> common::ToolSpec {
    common::ToolSpec {
        name: meta.id.clone(),
        version: meta.version.clone(),
        description: meta.description.clone(),
        input_schema: meta.input_schema.clone(),
        output_schema: meta.output_schema.clone(),
    }
}

/// Load a system prompt file, stripping SKILL.md frontmatter if present.
pub fn load_system_prompt(path: &str) -> Result<String, common::AppError> {
    let mut resolved_path = std::path::PathBuf::from(path);
    if !resolved_path.exists() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_path = std::path::PathBuf::from(manifest_dir)
                .join("../..")
                .join(path);
            if workspace_path.exists() {
                resolved_path = workspace_path;
            }
        }
    }
    if !resolved_path.exists() {
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let check_path = dir.join(path);
            if check_path.exists() {
                resolved_path = check_path;
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let content = std::fs::read_to_string(&resolved_path).map_err(|e| {
        common::AppError::internal(format!(
            "failed to read prompt file {:?}: {}",
            resolved_path, e
        ))
    })?;
    Ok(strip_frontmatter(&content))
}

fn strip_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content.to_string();
    }
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\r').unwrap_or(after_open);
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    let Some(close_idx) = after_open.find("\n---") else {
        return content.to_string();
    };
    let body_start = close_idx + 4;
    after_open[body_start..].trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rag_mode_config_deserializes_with_tool_pool_and_clusters() {
        let config = load_mode_config("rag").expect("rag mode should load");
        assert_eq!(config.id, "rag");
        assert!(
            config.tool_pool.is_empty(),
            "RAG retrieve tools are on-demand via memory cluster disclosure"
        );
        let codegen = config
            .skill_catalog
            .cluster_by_id("codegen")
            .expect("codegen cluster");
        assert!(codegen.atomic);
        assert_eq!(codegen.skills, vec!["codegen".to_string()]);
        assert!(
            config
                .skill_catalog
                .mandatory
                .synthesis
                .contains(&"rag-answer".to_string())
        );
    }

    #[test]
    fn search_mode_config_has_search_cluster() {
        let config = load_mode_config("search").expect("search mode should load");
        assert!(config.tool_pool.contains(&"web_search".to_string()));
        assert!(config.skill_catalog.cluster_by_id("search").is_some());
    }

    #[test]
    fn chat_mode_config_has_empty_retrieve_tool_pool() {
        let config = load_mode_config("chat").expect("chat mode should load");
        assert!(
            config.tool_pool.is_empty(),
            "chat memory tools are on-demand via memory cluster disclosure"
        );
        assert!(
            config
                .skill_catalog
                .mandatory
                .synthesis
                .contains(&"chat".to_string())
        );
    }

    #[test]
    fn skill_catalog_yaml_ids_exist_in_registry() {
        for mode in ["rag", "search", "chat"] {
            let config = load_mode_config(mode).expect("mode should load");
            let registry = crate::agents::progressive::PromptRegistry::standard_cached();
            for cluster in &config.skill_catalog.clusters {
                assert!(
                    registry.skill(&cluster.id).is_some(),
                    "mode {mode} cluster '{}' missing from registry",
                    cluster.id
                );
            }
            for skill in &config.skill_catalog.mandatory.synthesis {
                assert!(
                    registry.skill(skill).is_some(),
                    "mode {mode} mandatory synthesis '{skill}' missing from registry"
                );
            }
        }
    }

    #[test]
    fn legacy_flat_skill_catalog_deserializes() {
        let yaml = r#"
mode: test
system_prompt_base: prompts/orchestrators/chat-system.md
skill_catalog:
  - foo
  - bar
budget:
  max_iterations: 2
"#;
        let mut config: ModeConfig = serde_yaml::from_str(yaml).unwrap();
        config.normalize();
        assert_eq!(config.skill_catalog.flat_skill_ids().len(), 2);
    }

    #[test]
    fn rag_mode_has_mandatory_retrieve_codegen() {
        let config = load_mode_config("rag").expect("rag mode should load");
        assert!(config.inject_retrieval_query);
        assert!(config
            .skill_catalog
            .mandatory
            .retrieve
            .contains(&"codegen".to_string()));
    }

    #[test]
    fn search_mode_injects_retrieval_query() {
        let config = load_mode_config("search").expect("search mode should load");
        assert!(config.inject_retrieval_query);
    }

    #[test]
    fn chat_mode_no_retrieval_query_injection() {
        let config = load_mode_config("chat").expect("chat mode should load");
        assert!(!config.inject_retrieval_query);
    }
}
