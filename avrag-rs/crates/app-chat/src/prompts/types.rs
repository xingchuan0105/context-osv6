//! RAG plan/evaluation types.
#![cfg_attr(not(test), allow(dead_code))]

use serde::{Deserialize, Serialize};

use avrag_llm::LlmUsage;
use common::ToolCall;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RagStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    // Legacy fields (backward compat — kept as Option/default)
    #[serde(default)]
    pub recommendation: Option<StrategyRecommendation>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
    // New canonical fields
    pub decision: EvalDecision,
    #[serde(default)]
    pub next_actions: Vec<NextAction>,
    #[serde(default)]
    pub reasoning: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyDimension {
    pub name: String,
    #[serde(default)]
    pub attempted: bool,
    #[serde(default)]
    pub covered: bool,
    #[serde(default)]
    pub retrieved_count: usize,
    #[serde(default)]
    pub query_ids: Vec<String>,
    pub status: DimensionStatus,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyRecommendation {
    Synthesize,
    Replan,
    Broaden,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionStatus {
    CoveredStrong,
    CoveredWeak,
    Missing,
}

// ---------------- Search strategy evaluation ----------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchStrategyEvaluation {
    #[serde(default)]
    pub dimensions: Vec<StrategyDimension>,
    #[serde(default)]
    pub missing_dimensions: Vec<String>,
    #[serde(default)]
    pub weak_dimensions: Vec<String>,
    // Legacy fields (backward compat)
    #[serde(default)]
    pub recommendation: Option<SearchStrategyRecommendation>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub suggested_followup_queries: Vec<String>,
    // New canonical fields
    pub decision: EvalDecision,
    #[serde(default)]
    pub next_actions: Vec<NextAction>,
    #[serde(default)]
    pub reasoning: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStrategyRecommendation {
    Synthesize,
    Broaden,
    EscalateVertical,
}

// ---------------- Unified evaluation output ----------------

/// Decision emitted by Evaluate phase.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalDecision {
    /// Evidence sufficient, proceed to Answer.
    Sufficient,
    /// Evidence insufficient, replan with new actions.
    Insufficient,
    /// Give up, degrade gracefully.
    GiveUp,
}

/// Action the Evaluate phase recommends for replanning.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NextAction {
    SubQuery {
        query: String,
    },
    ToolCall {
        tool: String,
        args: serde_json::Value,
        reason: String,
    },
}

/// Per-sub-query item used to build the strategy evaluation prompt.
/// `tool_index` maps this sub-query back to the `tool_results` array so
/// result counts are reported against the correct tool call.
#[derive(Debug, Clone)]
pub(crate) struct SubQueryItem {
    pub id: String,
    pub text: String,
    pub tool_index: usize,
}

/// Plan strategy emitted by the PLAN phase LLM (P4 format).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStrategy {
    pub strategy: Vec<PlanStrategyItem>,
    #[serde(default = "default_next_step_str")]
    pub next_step: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStrategyItem {
    pub tool: String,
    #[serde(flatten)]
    pub params: serde_json::Value,
}

fn default_next_step_str() -> String {
    "answer".to_string()
}

#[derive(Debug, Clone)]
pub enum RagPlanDecision {
    ToolCalls(Vec<ToolCall>),
    Strategy(PlanStrategy),
    Clarify(String),
}

#[derive(Debug, Clone)]
pub struct RagPlanResult {
    pub decision: RagPlanDecision,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct RagAnswerResult {
    pub answer_text: String,
    pub llm_usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct RagBehaviorSkill {
    pub name: String,
    pub instructions: Vec<String>,
}

impl RagBehaviorSkill {
    pub(crate) fn new(
        name: impl Into<String>,
        instructions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            instructions: instructions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RagContext {
    pub mode: String,
    pub current_task: String,
    pub authoritative_context: String,
    pub reference_context: String,
    pub user_preference_memory: String,
    pub skill: RagBehaviorSkill,
    pub output_contract: String,
}

