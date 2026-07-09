---
name: heavytail-refine
description: "HeavyTail 精修阶段决策指南：何时 revise / research / finish（WriteRefine Agent Loop Skill）"
category: "writing-style"
disclose_at: retrieve
activation_phase: plan_and_evaluate
applicable_strategies: ["write_refine"]
---

## 何时调用 `write_refine_lexical`

- 诊断 ✗ **词汇重复度（Hapax）** 或 ✗ **词频分布（Zipf）**，且「词汇操作参考」非空。
- `repeat_term`：在缺 `term` 的句子里复用主题词（配合附录 reservoir）。
- `replace_term`：把过高频词 `from` 换成 `to`（削 Zipf 峰）。
- 词汇编辑与 `write_refine_revise` 一样计入有效 revise 轮。

## 何时调用 `write_refine_revise`

- 诊断报告显示 Band 未全过，且优先句/词清单非空。
- 你有明确的改写方案（每个 patch 改一个 `s<id>` 整句，以 `。！？` 结尾）。
- 一次 revise 集中改最影响 Band 的 3–8 句，不要散改。
- revise 有效轮上限 5；每个 patch 失败可重试，失败不计入有效轮。

## 何时调用 `write_refine_research`

- 背景附录中没有支撑某个关键事实的卡片，而你想在正文中使用它。
- 诊断显示词汇重复度（Hapax）过低，需要补充主题词的素材来源。
- 全程上限 5 次（与初稿调研分开记账）；第 6 次返回 `budget_exhausted`。
- 子 worker 自带 `max_iterations: 2`、`per_research_worker_tokens: 4000`，预算更小。
- observation 只返回 ≤3 张新卡 + 术语列表，不是全文。

## 何时调用 `write_refine_finish`（软结束）

- 四项 Band 全过 → 立即 finish。
- 若编排器开启 **核心 band 门禁**，hapax/zipf 仍 ✗ 时 `finish` 会被拒绝，需继续 revise/lexical。
- Band 未全过但可读性已足够（句长有起伏、用词不空洞、节奏成簇）。
- 预算将尽（ReAct 轮次接近 8、refine tokens 接近 40k、revise 有效轮接近 5）。
- `bands_satisfied` 字段仅作 telemetry，不作硬门禁——即使填 `false`，编排器仍会交付当前最优版本 + `validation_warning`。

## 不要做的事

- 不要直接输出最终正文——正文由编排器在 finish 后从 `DraftWorkspace` 取最优版本组装。
- 不要捏造关键事实——缺事实先查附录，再 `write_refine_research`；可改述、不可虚构。
- 不要引入新的 `[[n]]` 引用序号——引用由初稿与最终编排器组装。
- 不要输出 `<code>` 代码块——本 loop 无 codegen 路径。