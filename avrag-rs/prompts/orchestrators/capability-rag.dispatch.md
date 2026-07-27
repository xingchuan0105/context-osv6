---
name: capability-rag.dispatch
description: "Orchestrator-facing dispatch manual for the capability-RAG subagent (reader: orchestrator brain only; NOT injected into workers)."
---

## 给任务分配者

> 读者是 **orchestrator**；本段描述 **capability-RAG subagent**。

- **角色**：本 capability 的 **subagent**；在 **workspace** + **doc_scope** 上跑 ReAct retrieve，交 **handoff**，不写用户终答。
- **能做什么**：scope 内文档的 multi-hop 检索与抽取；代码侧计数/过滤。
- **不能做什么**：公网；observation 未出现的内容不当文档事实；检索不扩大到 doc_scope 外。
- **期望 brief 粒度**：一次 task brief ≈ 一个完整 research goal（可含多跳串联）；主检索在 subagent loop 内完成。Brief 建议含 `[goal]/[scope]`/`[deliverables]`/`[strategy]`/`[handoff]`（格式见 orchestrator-base）。
- **handoff**：`summary`、`key_facts`（evidence pointers；`basis`=observed|inferred）、`coverage`（full / partial / insufficient）、`gaps`、`premise_mismatch`（前提/归属与证据不符时 worker 的否决信号）。写 brief 时含 `[premise/归属核对]` 要求（见 orchestrator-base）。
- **re-dispatch**：orchestrator 仅在 gaps 仍指向未覆盖点时写更窄 brief；同 goal 连点帮助不大。
