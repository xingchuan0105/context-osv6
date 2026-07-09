//! Prompt-building methods for the WriteRefine ReAct loop.

use heavytail::diagnosis;
use heavytail::persona::{self, PersonaCard};

use crate::agents::r#loop::config::ModeConfig;

use super::helpers;
use super::WriteRefineLoopRunner;
use super::types::RefineContext;

impl<'a> WriteRefineLoopRunner<'a> {
    /// Build the per-round system prompt (base + iteration budget hint + round counter).
    pub(super) fn build_system_prompt(
        &self,
        mode: &ModeConfig,
        iteration: u8,
        max_iterations: u8,
        persona: Option<&PersonaCard>,
        revise_rounds_used: usize,
        research_calls_used: usize,
    ) -> String {
        let base = crate::agents::r#loop::config::load_system_prompt(&mode.system_prompt_base)
            .unwrap_or_default();
        let budget_hint = crate::agents::r#loop::assembler::build_iteration_budget_hint(
            iteration,
            max_iterations,
        );
        let round_counter = helpers::build_write_refine_round_counter_zh(
            iteration,
            max_iterations,
            revise_rounds_used,
            self.budget.max_rounds,
            research_calls_used,
            self.budget.max_on_demand_research,
            &self.budget,
        );
        // Inject mandatory skill bodies (heavytail-metrics) each round.
        let skills = self.render_mandatory_skills(mode);
        let mut out = base;
        if !skills.is_empty() {
            out.push_str("\n\n");
            out.push_str(&skills);
        }
        if let Some(p) = persona {
            out.push_str("\n\n");
            out.push_str(&persona::render_persona_system_zh(p));
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

    /// Render the mandatory retrieve skill bodies (heavytail-metrics every round).
    fn render_mandatory_skills(
        &self,
        mode: &ModeConfig,
    ) -> String {
        let registry = crate::agents::progressive::PromptRegistry::standard_cached();
        let mut out = String::new();
        for skill_id in &mode.skill_catalog.mandatory.retrieve {
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

    /// Render the per-round user message: diagnosis brief + canonical draft + appendix.
    pub(super) fn render_round_user_message(
        &self,
        ctx: &RefineContext,
        reservoir: &[String],
        first_round: bool,
        iteration: u8,
        max_iterations: u8,
        force_lexical_last_round: bool,
    ) -> String {
        let mut out = String::new();

        out.push_str(&helpers::build_write_refine_round_counter_zh(
            iteration,
            max_iterations,
            ctx.revise_rounds_used,
            self.budget.max_rounds,
            ctx.research_calls_used,
            self.budget.max_on_demand_research,
            &self.budget,
        ));
        out.push_str("\n");

        // 1. Diagnosis brief (metrics + data + priority sentences/words).
        let brief = diagnosis::render_diagnosis_brief_zh(&ctx.diagnosis, reservoir);
        // The legacy brief ends with a "## 你的任务" section instructing the
        // model to output patch lines. For the ReAct loop we replace that
        // section with a tool-call instruction.
        let brief = helpers::strip_task_section(&brief);
        out.push_str(&brief);
        out.push_str("\n\n");

        // 2. Numbered canonical draft.
        out.push_str("## 正文（编号）\n\n");
        out.push_str(&ctx.workspace.render_canonical());
        out.push_str("\n\n");

        // 3. Background appendix.
        out.push_str(&ctx.material_pack.render_appendix_zh());

        if let Some(ref p) = ctx.persona {
            let leaks = persona::check_persona_leakage(&ctx.workspace, p);
            if !leaks.is_empty() {
                out.push_str("\n## 人格泄漏警告\n\n");
                for hint in persona::render_leak_revise_hints(&leaks) {
                    out.push_str(&format!("- {hint}\n"));
                }
            }
        }

        // 4. Tool-call instruction (replaces the legacy "只输出 patch 行" task).
        out.push_str("## 你的任务\n\n");
        if force_lexical_last_round {
            out.push_str(
                "**最后一轮且 hapax/zipf 仍未过关**：本轮**只能**调用 `write_refine_lexical`。\n\
                 - hapax ✗ → `repeat_term` 复用词库词（`term` 取自附录词库）。\n\
                 - zipf ✗ → `replace_term` 将高频词替换为词库词。\n\
                 禁止调用 revise / research / finish。\n",
            );
        } else if first_round {
            out.push_str(
                "阅读上文指标、数据、优先清单和背景附录，调用**且仅调用**下列 tool 之一：\n\
                 - `write_refine_revise`：修改编号句子（`patches`）。\n\
                 - `write_refine_lexical`：`repeat_term` / `replace_term` 调词汇（hapax/zipf）。\n\
                 - `write_refine_research`：补检索（`kind`+`query`），全程上限 5 次。\n\
                 - `write_refine_finish`：收工（hapax/zipf 未过关时可能被拒绝）。\n\n\
                 关键事实不得捏造；缺事实先查附录，再调 `write_refine_research`。\n",
            );
        } else {
            out.push_str(
                "基于上轮 delta 与新优先清单，决定下一步：继续 `write_refine_revise`、\
                 `write_refine_lexical`、`write_refine_research`（剩余预算见 observation），\
                 或 `write_refine_finish` 收工。\n",
            );
        }

        out
    }
}
