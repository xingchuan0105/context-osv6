//! Ports so WriteRefine can live in write-core without depending on app-chat agents.

use async_trait::async_trait;
use contracts::ToolSpec;
use contracts::chat::ToolStatus;
use heavytail::persona::PersonaCard;
use heavytail::skeleton::MaterialCard;

use crate::RefineLoopBudget;

/// On-demand research vertical for WriteRefine `write_refine_research`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteResearchKind {
    Rag,
    Web,
}

/// Material extracted from one research worker call.
#[derive(Debug, Clone, Default)]
pub struct WriteResearchHit {
    pub cards: Vec<MaterialCard>,
}

/// Parent chat metadata needed by the refine loop (no full AgentRequest).
#[derive(Debug, Clone, Default)]
pub struct WriteParentMeta {
    /// Optional billing/product tier string for mode budget resolution.
    pub user_tier: Option<String>,
}

/// Research sub-workers (RAG / Web) used inside the refine loop.
#[async_trait]
pub trait WriteResearchPort: Send + Sync {
    async fn research(
        &self,
        kind: WriteResearchKind,
        query: &str,
        token_budget: usize,
    ) -> Result<WriteResearchHit, String>;
}

/// Observability sink for refine-loop activity / tool events.
#[async_trait]
pub trait WriteActivitySink: Send + Sync {
    async fn activity(&self, stage: &str, message: String);
    async fn tool_call(&self, tool: &str, args: Option<serde_json::Value>);
    async fn tool_result(
        &self,
        tool: &str,
        status: ToolStatus,
        data: Option<serde_json::Value>,
    );
}

/// Loads write_refine mode tools, temperature, iteration budget, and system prompt.
///
/// Implemented in app-chat against ModeConfig / CapabilityRegistry / PromptRegistry.
pub trait WriteRefineModeHost: Send + Sync {
    fn temperature(&self) -> f32;
    fn tool_specs(&self) -> Vec<ToolSpec>;
    fn max_react_iterations(&self, user_tier: Option<&str>, hard_cap: u8) -> u8;
    fn system_prompt(
        &self,
        iteration: u8,
        max_iterations: u8,
        persona: Option<&PersonaCard>,
        revise_rounds_used: usize,
        research_calls_used: usize,
        budget: &RefineLoopBudget,
    ) -> String;
}
