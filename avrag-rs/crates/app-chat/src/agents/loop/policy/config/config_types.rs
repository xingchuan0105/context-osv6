use std::collections::HashMap;

use super::skill_catalog::SkillCatalogConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeConfig {
    #[serde(alias = "mode")]
    pub id: String,
    pub system_prompt_base: String,
    /// Tool ids disclosed to the LLM during retrieve. Schemas resolved from
    /// [`CapabilityRegistry`](crate::agents::capability::CapabilityRegistry).
    #[serde(default)]
    pub tool_pool: Vec<String>,
    #[serde(default, deserialize_with = "super::skill_catalog::deserialize_skill_catalog")]
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
