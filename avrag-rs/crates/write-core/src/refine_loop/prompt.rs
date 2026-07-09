//! Per-round user message assembly for WriteRefine (system prompt via ModeHost).

use heavytail::diagnosis;
use heavytail::persona;

use crate::refine_helpers;
use crate::refine_types::RefineContext;

use super::WriteRefineLoopRunner;

impl<'a> WriteRefineLoopRunner<'a> {
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

        out.push_str(&refine_helpers::build_write_refine_round_counter_zh(
            iteration,
            max_iterations,
            ctx.revise_rounds_used,
            self.budget.max_rounds,
            ctx.research_calls_used,
            self.budget.max_on_demand_research,
            &self.budget,
        ));
        out.push_str("\n");

        let brief = diagnosis::render_diagnosis_brief_zh(&ctx.diagnosis, reservoir);
        let brief = refine_helpers::strip_task_section(&brief);
        out.push_str(&brief);
        out.push_str("\n\n");

        out.push_str("## 正文（编号）\n\n");
        out.push_str(&ctx.workspace.render_canonical());
        out.push_str("\n\n");

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
