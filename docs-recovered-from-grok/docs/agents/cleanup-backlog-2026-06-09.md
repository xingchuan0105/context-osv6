# 架构清理 Backlog 2026-06-09（B 组：已定决策收尾 / 一致性修复）

> 来源：`/improve-codebase-architecture` 审查（聚焦 main agent 路径、ReAct loop、渐进披露）。
> 本文档收录**低风险、不触碰 ADR-0007 提议范围**的收尾项——多为已定决策（D1–D14、ADR-0007 §10.4）尚未落地的代码漂移，或层间命名不一致。
> A 组（架构深化）见 `agent-deepening-plan-2026-06-09.md`（**✅ 已实现**）。

**Review 状态（2026-06-09 二次 review + D8 收尾）**：**5 项全部 ✅ 完成**。

## 优先级总览

| # | 项 | 状态 |
|---|-----|------|
| 5 | `tool_pool` 单一配置 + search schema 迁入 CapabilityRegistry（D8） | ✅ |
| 6 | `AgentRequest.session_summary` 幽灵字段 | ✅ |
| 7 | routing telemetry `strategy_id` → `mode_id` | ✅ |
| 8 | 陈旧契约测试 | ✅ |
| 9 | `prompts/atomic-tools/*` 是否孤儿 | ✅ 核实闭环 |

---

## 5. `native_tools` / `tool_pool` / `tool_definitions` 三名一物 — ✅ 完成（D8）

**已完成（2026-06-09）**
- 删除 `ModeConfig.tool_definitions` 及 `native_tools` serde alias。
- `resolve_tool_specs` 仅从 `CapabilityRegistry::tool` 解析。
- `CapabilityRegistry::standard()` 注册 `web_search` / `web_fetch`（schema 来自 `SkillComponent::spec()`，与 runtime 执行同源）。
- `modes/search.yaml` 仅保留 `tool_pool: [web_search, web_fetch]`。

**验证**：`search_mode_resolves_tool_pool_from_capability_registry`；`cargo test -p app --lib`。

---

## 6. `AgentRequest.session_summary` 幽灵字段 — ✅ 完成

**证据**：`runtime.rs` 的 `AgentRequest` 已无 `session_summary` 字段。

**说明**：`build_session_summary` / `update_session_summary` 等 chat-memory 产品功能仍合法存在，与 agent ReAct 注入路径无关（ADR-0007 §2.4 废弃的是 agent system prompt 注入，非 PG 摘要存储）。

---

## 7. routing telemetry 命名 — ✅ 完成

**证据**：全仓 `strategy_id` 零命中（audit payload 已改 `mode_id`）。

---

## 8. 陈旧契约测试 — ✅ 完成

**证据**
- `chat_conversation_history_tools_in_catalog` 已移除。
- `agent_catalog_contract.rs` 通过；注释说明 conversation_history 经 memory 簇 / PG 路径披露。

---

## 9. `prompts/atomic-tools/*` 是否孤儿 — ✅ 核实闭环

**结论（ADR-0007 预期，非 bug）**

| 路径 | PromptRegistry | 运行时披露 |
|------|----------------|------------|
| `prompts/clusters/*` | ✅ build.rs 扫描加载 | 经 DisclosurePlanner |
| `prompts/synthesis/`、`orchestrators/` | ✅ 扫描加载 | synthesis / system base |
| `prompts/atomic-tools/*` | ❌ **故意不扫描** | 不经 PromptRegistry |

**运行时替代路径**
- Native tool **schema**：`modes/*.yaml` 的 `tool_pool` → `CapabilityRegistry::tool`（D8 已落地）。
- Native tool **执行**：`agents/skills/builtin/*`（如 `conversation_history_load` / `web_search`）经 `SkillComponent` 注册，`unified/helpers.rs` 分发。
- `conversation_history_*`：不再经 atomic-tool catalog；memory 簇 + PG `prior_turns` 承担跨轮连续性。

**证据**
- `build.rs` 仅扫描 `clusters/`、`synthesis/`、`orchestrators/`（已加注释）。
- `prompt_registry.rs` 测试：
  - `atomic_retrieval_tools_are_not_prompt_skills`
  - `atomic_tools_directory_skills_are_not_in_prompt_registry`（新增：conversation-history-load、web_search 等）
  - `memory` / `codegen` 等簇 skill 可检索。

**处置**：`prompts/atomic-tools/` 保留为**参考文档 / 迁移前 artifact**，不要求接入 PromptRegistry。若需清理目录，单独开「文档归档」PR，不影响运行时。

---

*Updated: 2026-06-09 · A 组见 `agent-deepening-plan-2026-06-09.md`（已实现）。*
