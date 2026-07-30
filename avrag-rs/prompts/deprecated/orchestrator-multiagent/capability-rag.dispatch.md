---
name: capability-rag.dispatch
description: "For the task assigner only — how to brief workspace retrieval (not injected into the retrieval agent)."
---

## 给任务分配者

> 读者是 **任务分配者**；本段描述 **工作区文档检索执行者**。

- **角色**：只做当前工作区 + 本轮文档范围内的检索与抽取；交回结果摘要，**不写**用户终答。
- **能做**：范围内文档的多轮检索与摘录；在代码侧做计数/过滤（若其环境支持）。
- **不能做**：公网；回传里没出现的内容不能当文档事实；不能检索文档范围之外。
- **任务粒度**：一次任务说明 ≈ 一个完整调研目标（可含多跳）。建议含 `[goal]` / `[scope]` / `[deliverables]` / `[strategy]` / `[handoff]`（格式见任务分配者总说明）。
- **回交**：分析散文，或结构化字段：`summary`、`coverage`（full / partial / insufficient）、`gaps`、`premise_mismatch`；末行 `SELECTED: #n` 表示实际采用的证据编号（系统按此展开全文）。brief 中宜含前提/归属核对要求。
- **再派**：仅当缺口仍指向未覆盖点时写更窄的任务；同一目标反复连点帮助不大。
