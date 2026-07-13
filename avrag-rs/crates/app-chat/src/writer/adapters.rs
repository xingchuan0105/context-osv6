//! app-chat adapters for write-core ports (research / activity / mode host).

use std::time::Duration;

use async_trait::async_trait;
use contracts::ToolSpec;
use contracts::chat::ToolStatus;
use heavytail::persona::PersonaCard;
use write_core::{
    RefineLoopBudget, WriteActivitySink, WriteParentMeta, WriteRefineModeHost, WriteResearchHit,
    WriteResearchKind, WriteResearchPort,
};

use agent_loop::events::{AgentEvent, AgentEventSink};
use agent_loop::r#loop::assembler::build_iteration_budget_hint;
use agent_loop::r#loop::config::{load_mode_config, load_system_prompt, ModeConfig};
use agent_tools::progressive::PromptRegistry;
use agent_loop::runtime::AgentRequest;
use crate::agents::AgentKind;
use crate::writer::cards;
use crate::writer::invoker::SubagentInvoker;

/// Map parent AgentRequest → write-core meta.
pub fn parent_meta_from_request(parent: &AgentRequest) -> WriteParentMeta {
    WriteParentMeta {
        user_tier: parent
            .metadata
            .get("user_tier")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

/// Bridge AgentEventSink → WriteActivitySink.
pub struct AgentWriteActivitySink<'a> {
    pub inner: &'a dyn AgentEventSink,
}

#[async_trait]
impl WriteActivitySink for AgentWriteActivitySink<'_> {
    async fn activity(&self, stage: &str, message: String) {
        let _ = self
            .inner
            .emit(AgentEvent::Activity {
                stage: stage.to_string(),
                message,
                detail: None,
                counts: Default::default(),
                sources_preview: Vec::new(),
            })
            .await;
    }

    async fn tool_call(&self, tool: &str, args: Option<serde_json::Value>) {
        let _ = self
            .inner
            .emit(AgentEvent::ToolCall {
                tool: tool.to_string(),
                args,
            })
            .await;
    }

    async fn tool_result(&self, tool: &str, status: ToolStatus, data: Option<serde_json::Value>) {
        let _ = self
            .inner
            .emit(AgentEvent::ToolResult {
                tool: tool.to_string(),
                status,
                data,
                elapsed_ms: 0,
            })
            .await;
    }
}

/// On-demand research via SubagentInvoker + card extraction.
pub struct SubagentResearchPort<'a> {
    pub invoker: &'a SubagentInvoker,
    pub parent: &'a AgentRequest,
}

#[async_trait]
impl WriteResearchPort for SubagentResearchPort<'_> {
    async fn research(
        &self,
        kind: WriteResearchKind,
        query: &str,
        token_budget: usize,
    ) -> Result<WriteResearchHit, String> {
        let agent_kind = match kind {
            WriteResearchKind::Rag => AgentKind::Rag,
            WriteResearchKind::Web => AgentKind::Search,
        };
        let mut worker_req =
            SubagentInvoker::worker_request(self.parent, agent_kind, query);
        worker_req.max_iterations = Some(2);
        worker_req.query = query.to_string();

        let result = self
            .invoker
            .run_worker(worker_req, token_budget, Duration::from_secs(60))
            .await
            .map_err(|e| e.to_string())?;

        let guard = self.parent.guard_pipeline.as_deref();
        let trace_id = self.parent.session_id.as_deref();
        let extraction = cards::extract_material_cards(&result, agent_kind, guard, trace_id);
        Ok(WriteResearchHit {
            cards: extraction.cards,
        })
    }
}

/// ModeConfig-backed host for write_refine tools / prompts / tier budget.
pub struct AppWriteRefineMode {
    mode: ModeConfig,
}

impl AppWriteRefineMode {
    pub fn load() -> Result<Self, String> {
        let mode = load_mode_config("write_refine").map_err(|e| e.to_string())?;
        Ok(Self { mode })
    }
}

impl WriteRefineModeHost for AppWriteRefineMode {
    fn temperature(&self) -> f32 {
        self.mode.temperature.unwrap_or(0.4)
    }

    fn tool_specs(&self) -> Vec<ToolSpec> {
        // Write control tools are local specs — not ToolCatalog / SkillRegistry.
        agent_tools::skills::builtin::write_refine::tool_specs_for_pool(&self.mode.tool_pool)
    }

    fn max_react_iterations(&self, user_tier: Option<&str>, hard_cap: u8) -> u8 {
        let tier_val = user_tier.map(|s| serde_json::Value::String(s.to_string()));
        let tier_iter = self
            .mode
            .budget
            .resolve_max_iterations(tier_val.as_ref());
        tier_iter.min(hard_cap)
    }

    fn system_prompt(
        &self,
        iteration: u8,
        max_iterations: u8,
        persona: Option<&PersonaCard>,
        revise_rounds_used: usize,
        research_calls_used: usize,
        budget: &RefineLoopBudget,
    ) -> String {
        let base = load_system_prompt(&self.mode.system_prompt_base).unwrap_or_default();
        let budget_hint = build_iteration_budget_hint(iteration, max_iterations);
        let round_counter = write_core::build_write_refine_round_counter_zh(
            iteration,
            max_iterations,
            revise_rounds_used,
            budget.max_rounds,
            research_calls_used,
            budget.max_on_demand_research,
            budget,
        );
        let skills = self.render_mandatory_skills();
        let mut out = base;
        if !skills.is_empty() {
            out.push_str("\n\n");
            out.push_str(&skills);
        }
        if let Some(p) = persona {
            out.push_str("\n\n");
            out.push_str(&heavytail::persona::render_persona_system_zh(p));
            out.push_str(
                "\n\n**内化人格：影响措辞与取舍，禁止在正文自我介绍或引用小传事实**",
            );
        }
        out.push_str("\n\n");
        out.push_str(&budget_hint);
        out.push_str("\n\n");
        out.push_str(&round_counter);
        out
    }
}

impl AppWriteRefineMode {
    fn render_mandatory_skills(&self) -> String {
        let registry = PromptRegistry::standard_cached();
        let mut out = String::new();
        for skill_id in &self.mode.skill_catalog.mandatory.retrieve {
            if let Some(skill) = registry.skill(skill_id) {
                out.push_str(&format!(
                    "## Skill: {id} (v{ver})\n{body}\n\n",
                    id = skill.id(),
                    ver = skill.version(),
                    body = skill.system_prompt().trim()
                ));
            }
        }
        out
    }
}
