---
name: capability-search.dispatch
description: "Orchestrator-facing dispatch manual for the websearch subagent (reader: orchestrator brain only; NOT injected into workers)."
---

## 给任务分配者

> 读者是 **orchestrator**；本段描述 **websearch subagent**。

- **角色**：本 capability 的 **subagent**；公网 web search / fetch 的 ReAct loop，交 **handoff**，不写用户终答。
- **能做什么**：实时 / 外部网页信息。
- **不能做什么**：workspace 私有文档；检索结果未出现的内容不当网页事实。
- **期望 brief 粒度**：一次 task brief ≈ 一个完整 research goal（可含多角度子查询）；主检索在 subagent loop 内完成。Brief 建议含 `[goal]`/`[scope]`/`[deliverables]`/`[strategy]`/`[handoff]`（格式见 orchestrator-base）；`[scope]` 写可独立成立的 query theme（不依赖 workspace 内未消解指代；中英可检索表述更稳）。
- **handoff**：`summary`、`key_facts`（来源指针）、`coverage`、`gaps`。
- **re-dispatch**：仅针对仍未覆盖的 gaps 换更具体 theme；连续空结果运行时会收敛，换皮空转帮助不大。
