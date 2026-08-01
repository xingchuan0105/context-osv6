use std::collections::HashMap;

use super::skill_catalog::SkillCatalogConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeConfig {
    #[serde(alias = "mode")]
    pub id: String,
    pub system_prompt_base: String,
    /// Tool ids disclosed to the LLM during retrieve. Schemas resolved from
    /// [`CapabilityRegistry`](agent_tools::capability::CapabilityRegistry).
    #[serde(default)]
    pub tool_pool: Vec<String>,
    #[serde(
        default,
        deserialize_with = "super::skill_catalog::deserialize_skill_catalog"
    )]
    pub skill_catalog: SkillCatalogConfig,
    /// Inject retrieval/display query block during retrieve (and synthesis when true).
    #[serde(default)]
    pub inject_retrieval_query: bool,
    pub budget: BudgetConfig,
    pub auto_fallback: Option<AutoFallbackConfig>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub loop_exit: LoopExitConfig,
    #[serde(default)]
    pub synthesis_output: SynthesisOutputConfig,
    /// U3: this loop is a channel **worker** whose final message is the
    /// internal handoff JSON (set by app-chat `apply_worker_handoff_loop_exit`).
    /// Serde-default false so yamls and older configs are unaffected.
    #[serde(default)]
    pub worker_handoff: bool,
    /// SaC SDK methods the sandbox may call (A3). Empty = no gate (tests/legacy).
    /// Product `assemble_mode` always fills this from capability flags.
    #[serde(default)]
    pub sdk_primitives: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoopExitConfig {
    /// **Skill-owned only** (no host hard gate). When true in YAML it documents
    /// product intent for capability/skill prose; the loop does not refuse
    /// DirectAnswer or force degraded for missing answer-grade chunks.
    #[serde(default)]
    pub require_evidence: bool,
    #[serde(default)]
    pub allow_content_early_stop: bool,
    #[serde(default)]
    pub skip_synthesis_on_direct_answer: bool,
}

impl Default for LoopExitConfig {
    fn default() -> Self {
        Self {
            // Host no longer enforces; default false avoids implying a hard gate.
            require_evidence: false,
            allow_content_early_stop: false,
            skip_synthesis_on_direct_answer: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerContractKind {
    InternalAnswerV1,
    InternalSearchAnswerV1,
    /// Unified doc+web synthesis (`[[cite:…]]` + `[[web:n]]`). Preferred for rag/search/dual.
    InternalAnswerUnifiedV1,
    /// @deprecated alias of unified dual path; still accepted in old configs.
    InternalHybridAnswerV1,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetConfig {
    /// Safety ceiling on retrieve completes (prevents infinite loops when
    /// usage is missing). Prefer `max_tokens` as the primary cost control.
    pub max_iterations: u8,
    #[serde(default)]
    pub by_user_tier: Option<HashMap<String, u8>>,
    /// Primary retrieve budget: cumulative LLM `total_tokens` (prompt+completion).
    /// `None` / omitted → rounds-only (legacy).
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub max_tokens_by_user_tier: Option<HashMap<String, u32>>,
    /// Optional absolute override for no-chunk continue token boost.
    /// When `None` (preferred), boost = **50% of baseline** `max_tokens`
    /// (product rule: continue budget is not “+N free rounds”).
    #[serde(default)]
    pub no_chunk_grace_tokens: Option<u32>,
}

/// Continue / no-chunk grace: fraction of **baseline** token budget added once.
pub const CONTINUE_TOKEN_BUDGET_RATIO_NUM: u32 = 1;
pub const CONTINUE_TOKEN_BUDGET_RATIO_DEN: u32 = 2; // 50%

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_iterations: 4,
            by_user_tier: None,
            max_tokens: None,
            max_tokens_by_user_tier: None,
            // Prefer ratio-based grace (50% baseline); absolute is ops override only.
            no_chunk_grace_tokens: None,
        }
    }
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

    /// Resolved token cap. `0` means unlimited (rounds-only).
    pub fn resolve_max_tokens(&self, request_tier: Option<&serde_json::Value>) -> u32 {
        let tier_str = request_tier
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let resolved = if let Some(tier) = tier_str {
            self.max_tokens_by_user_tier
                .as_ref()
                .and_then(|m| m.get(&tier).copied())
                .or(self.max_tokens)
        } else {
            self.max_tokens
        };
        resolved.unwrap_or(0)
    }

    /// Tokens added once when retrieve budget hits with zero answer-grade chunks.
    /// Default: `⌊baseline_max_tokens × 50%⌋`. Absolute `no_chunk_grace_tokens`
    /// overrides when set. Baseline `0` (rounds-only) → absolute or `0`.
    pub fn resolve_continue_token_boost(&self, baseline_max_tokens: u32) -> u32 {
        if let Some(abs) = self.no_chunk_grace_tokens {
            return abs;
        }
        if baseline_max_tokens == 0 {
            return 0;
        }
        baseline_max_tokens.saturating_mul(CONTINUE_TOKEN_BUDGET_RATIO_NUM)
            / CONTINUE_TOKEN_BUDGET_RATIO_DEN
    }

    /// @deprecated name — use [`Self::resolve_continue_token_boost`].
    pub fn resolve_no_chunk_grace_tokens(&self) -> u32 {
        self.resolve_continue_token_boost(self.max_tokens.unwrap_or(0))
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
