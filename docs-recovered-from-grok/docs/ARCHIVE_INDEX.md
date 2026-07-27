# 📁 Documentation Archive

This directory contains historical project documentation.

## ⚠️ 归档说明

以下文档为 **2026-04-26 审查期间** 产生的分析文档，部分结论和状态已随代码迭代而过时。保留用于历史追溯，**不应作为当前架构决策的单一依据**。

## 归档文档清单

### 2026-04-26 审查批次（in-place 标注 ARCHIVED）

| 文档 | 产生日期 | 状态 | 备注 |
|------|----------|------|------|
| ARCHITECTURE_ISSUES_P0_2026-04-26.md | 2026-04-26 | 部分过时 | P0 项大部分已修复，见 CODE_REVIEW_2026-05-06.md |
| DEEP_ARCHITECTURE_REVIEW_2026-04-26.md | 2026-04-26 | 部分过时 | 架构分析仍有效，具体 GAP 状态需交叉核对 |
| FIX_SUMMARY_2026-04-26.md | 2026-04-26 | 历史参考 | 修复记录，后续修复见 CODE_REVIEW_2026-05-06.md |
| GAP_ANALYSIS_ARCHITECTURE_2026-04-26.md | 2026-04-26 | 部分过时 | GAP 清单已更新 |
| REFINED_FIX_PLAN_2026-04-26.md | 2026-04-26 | 历史参考 | 计划文档 |
| T2RAG_FINAL_PLAN_2026-04-26.md | 2026-04-26 | 历史参考 | T2RAG 计划 |
| T2RAG_REFINED_PLAN_2026-04-26.md | 2026-04-26 | 历史参考 | T2RAG 细化计划 |

### 2026-06-09 架构审核设计文档（已迁至 `docs/archive/`）

**归档原因：** 6 份设计文档的 Status 行均为 `Proposed (待实现)`，但实际已于 2026-06-09 commit `1c89852` / `ba05601` 实现完毕，由
`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md` 作为统一交付记录代替。本批归档于 2026-06-13 Brooks-Lint 评审时执行。

| 原路径 | 现路径 | 实现状态 |
|--------|--------|----------|
| docs/agents/loop-optimizer-design.md | docs/archive/loop-optimizer-design.md | ✅ 已实现 `avrag-rs/crates/app-chat/src/agents/loop/optimizer.rs` |
| docs/agents/v5-state-machine-cleanup-design.md | docs/archive/v5-state-machine-cleanup-design.md | ✅ 已删除 `rig_adapter.rs` / `LoopBudget` 旧字段 / `StateRecord` / `AgentEvent::StateTransition` |
| docs/agents/router-policy-removal-design.md | docs/archive/router-policy-removal-design.md | ✅ 已删除 `capability/router.rs`，简化 `unified::mod` |
| docs/agents/schema-terminology-alignment-design.md | docs/archive/schema-terminology-alignment-design.md | ✅ `StrategySchema → ModeSchema`、`strategy_id → mode_id`、`api_version v5 → v6` |
| docs/agents/frontend-mapping-cleanup-design.md | docs/archive/frontend-mapping-cleanup-design.md | ✅ 删除 `RawWorkspace*` / `mapWorkspace*` |
| docs/agents/frontend-chat-god-component-design.md | docs/archive/frontend-chat-god-component-design.md | ✅ `workspace-chat-pane.tsx` 2514 → 174 行；抽出 `useChatSession` / `ChatComposer` / `ChatMessageList` |

> 查阅最新事实请优先看 `docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`；归档文档保留供历史追溯（含原始动机、决策表、风险矩阵）。

## 当前有效文档

| 文档 | 说明 |
|------|------|
| CODE_REVIEW_2026-05-06.md | 较新的全面审查报告，覆盖到 2026-05-06 GAP 状态 |
| CODEBASE_HEALTH_DASHBOARD_2026-06-11.md | Brooks-Lint 四维度健康评估（最新综合分数 ~74） |
| HEALTH_OPTIMIZATION_HANDOFF_2026-06-11.md | T1-T16 优化阶段交付报告，含遗留项与依赖图 |
| t13-app-split-inventory.md | `app` crate 拆分基线与 Phase 2 迁移记录 |
| agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md | 2026-06-09 七项架构改进的实施总结（替代本批归档设计文档） |
| agents/agent-deepening-plan-2026-06-09.md | A 组（disclosure + run god-method）落地文档（已实现） |
| agents/cleanup-backlog-2026-06-09.md | B 组（命名一致性 / D8 收尾）（已实现） |
| avrag-rs/docs/adr/ | 架构决策记录（ADR），持续维护 |
| avrag-rs/prompts/ | 外置化 Prompt 模板 |

## 维护约定

1. 新审查报告产生后，旧报告应标注 `ARCHIVED` 并移入 `docs/archive/`（或 in-place 标注）
2. 任何引用旧文档的代码注释应更新为指向最新文档
3. 每季度清理一次归档文档，确认是否仍有参考价值
4. 设计文档（`*-design.md`）实现后：标注 Status: Implemented 并 `mv` 到 `docs/archive/`，在本索引中追加映射行

---
*最近更新: 2026-06-13（Brooks-Lint review 同步归档六份 2026-06-09 已实现设计文档）*
