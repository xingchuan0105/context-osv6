---
name: heavytail-refine
description: "When to revise, research, or finish during writing refine"
category: "writing-style"
disclose_at: retrieve
activation_phase: plan_and_evaluate
applicable_strategies: ["write_refine"]
---

## 何时 `write_refine_lexical`

- 诊断显示 **词汇重复度** 或 **词频分布** 不达标，且「词汇操作参考」非空。
- `repeat_term`：在缺该词的句子里复用主题词（可对照附录词库）。
- `replace_term`：把过高频词换成给定替代词。
- 词汇编辑与句级改写一样计入有效改写轮。

## 何时 `write_refine_revise`

- 诊断未全过，且优先句/词清单非空。
- 每个补丁改一个 `s<编号>` 整句，以 `。！？` 结尾。
- 一次集中改最影响指标的 3–8 句。
- 有效改写轮上限 5；失败可重试且不计入有效轮。

## 何时 `write_refine_research`

- 附录里没有支撑某关键事实的卡片，而正文需要它。
- 词汇过散、需要补充主题词素材时。
- 全程上限 5 次；第 6 次会返回额度用尽。
- 补检索回传通常只有少量摘要卡片与术语，不是全文。

## 何时 `write_refine_finish`

- 四项指标都达标 → 立即 finish。
- 若开启了「核心指标门禁」，词汇相关项未达标时 finish 可能被拒，需继续改写。
- 未全过但可读性已够（句长有起伏、用词不空、节奏成簇）→ 可以 finish。
- 额度将尽（总轮次、token、有效改写轮接近上限）→ 可以 finish。
- `bands_satisfied` 仅作记录；即使填 false，仍可能交付当前最优版并带警告。

## 边界

- 不要直接输出整篇终稿；finish 后由流程取当前最优版组装。
- 不要虚构事实；缺事实先查附录，再补检索。
- 不要新增引用序号；引用由初稿与最终组装负责。
- 不要输出检索用 `<code>` 块；本精修循环没有该路径。
