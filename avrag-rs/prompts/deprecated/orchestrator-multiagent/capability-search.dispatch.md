---
name: capability-search.dispatch
description: "For the task assigner only — how to brief web search (not injected into the search agent)."
---

## 给任务分配者

> 读者是 **任务分配者**；本段描述 **网页检索执行者**。

- **角色**：只做公网搜索与打开页面；交回结果摘要，**不写**用户终答。
- **能做**：时效信息、外部网页事实。
- **不能做**：工作区私有文档；检索回传未出现的内容不能当网页事实。
- **任务粒度**：一次任务说明 ≈ 一个完整调研目标（可含多角度子查询）。建议含 `[goal]` / `[scope]` / `[deliverables]` / `[strategy]` / `[handoff]`；`[scope]` 写可独立成立的查询主题（不依赖工作区里未消解的指代；中英表述通常更稳）。
- **回交**：分析散文，或 `summary` / `coverage` / `gaps` / `premise_mismatch`；末行 `SELECTED: #n` 为采用的证据编号。brief 中宜含前提/归属核对要求。
- **再派**：只针对仍未覆盖的缺口换更具体主题；连续空结果后换皮空转帮助不大。
