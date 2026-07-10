# TN 代码质量整改交接文档（2026-07-09）

| 字段 | 值 |
|------|-----|
| 状态 | **TN-3 S4 Done**；Product App Phase A+B+C Done（S0–S5） |
| 分支 | 本地 `master`（solo trunk） |
| 范围 | `avrag-rs` / `contracts` / `frontend_next` / scripts（**不含** `frontend_rust`） |
| 主方案 | [`TN_CODE_QUALITY_REMEDIATION_2026-07-09.md`](./TN_CODE_QUALITY_REMEDIATION_2026-07-09.md) |
| TN-3 plan | [`TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md`](./TN3_P0_P5_AND_TEST_PYRAMID_PLAN_2026-07-09.md) |
| **架构 Phase A** | [`PRODUCT_APP_ARCHITECTURE_MIGRATION_PLAN_2026-07-10.md`](./PRODUCT_APP_ARCHITECTURE_MIGRATION_PLAN_2026-07-10.md) |
| **TN Phase B** | [`PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md) — **Done** |
| **TN Phase C** | [`PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md) — **Done** |
| 产品词 | ADR-0006 **§5a**：Capability / Skill / Tool **三层保留** |

---

## 1. 一句话进度

**TN 主线 + TN-3 S4 已关。** 工具执行单点 `ToolCatalog`；Bound 面已拆；workspace 命名主体完成；UserProfile 强类型 merge；测试金字塔入口就绪（日常 L1）。

**架构 Phase A+B 已落地**（Conversation 单入口、Write 出 agent lane、write_refine 工具单一真相）。  
**架构 Phase A+B+C 已落地**（Conversation 直调 pipeline；write_refine 纯 ToolSpec）。  
Phase C：[`PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md) **Done**。

---

## 2. 已完成（摘要）

| 波次 | 状态 |
|------|------|
| Wave 0–6 + P0–P7 + TN-2 | Done |
| W1–W6 结构债 | Done |
| R1–R3 扫尾 | Done |
| **TN-3** | **Done** — P0–P4 结构 + P5 金字塔（入口/盘点/去重/L1 测时） |

---

## 3. 日常验证（Solo / L1）

```bash
bash scripts/check_file_size_limits.sh
# 推荐入口（落地后）：
# bash scripts/test-l1.sh
cd avrag-rs
cargo test -p agent-tools --lib
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
# 改到的 crate 再定向加测
pnpm -C frontend_next exec tsc --noEmit
```

**不进日常默认**：真 LLM、Playwright 全旅程、rag_quality、性能基线（L3 / nightly）。

---

## 4. 产品 / 工程约定（勿回退）

| 主题 | 说明 |
|------|------|
| Capability ≠ Skill ≠ Tool | ADR-0006 §5a |
| 执行单点 | `ToolCatalog` + `dispatch_tool` only |
| Capabilities API | mode `tool_pool` ∪ `auto_fallback.tool_id` |
| JSON / URL | **workspace** 用语；**仅** `/workspaces/*`（无 `/notebooks` 双挂、无长期 notebook alias） |
| Handler | Product Apps：`workspace`/`docs` / `agent` / `write_app` / `admin_*` / `share` / `prefs` / `billing_api` |
| **AppState 停增（P0 / ADR-0007）** | **禁止**向 `AppState` 新增业务方法。新能力进 `product_apps::*` 或 domain service。Bound 已拆除。 |
| Write | 不走 UnifiedAgent `ToolCatalog`；`write-core` 自有 refine dispatch |
| 不恢复 ExecutePlan | |
| Solo | 本地 trunk；定向测试；CI smoke 非默认阻塞 |

### 测试金字塔（产品拍板）

| 层 | 何时 | 内容 |
|----|------|------|
| L1 | 每次提交 | 编译、契约、crate lib、file-size |
| L2 | 动机制 / 波次 | mock 入库与四模式 smoke |
| L3 | 波次末 / 发版 | 短 UI 旅程；真 LLM 每模式 1–2 条；质量/性能分 job |

---

## 5. 非目标

| 项 | 说明 |
|----|------|
| C4 | 不做 |
| `frontend_rust` | 范围外 |
| 日常 PR 强绑真 LLM / 全 Playwright | 不做 |
| 性能进日常红线 | 不做（独立观测） |

---

## 6. 下一阶段入口（架构，非 TN 回归）

| 波次 | 交付 | 状态 |
|------|------|------|
| W0 | 立宪 + ADR + Bound freeze（T1） | **Done** |
| W1 | **ShareApp** 完整样板切片 | **Done** |
| W2–W6 | Workspace → Billing/Prefs → Admin → Agent/Write → 拆 Bound | **Done（Phase A）** |
| **R0–R5** | TN 结构修复（Phase B） | **Done** |
| **S0–S5** | 包装收口（Phase C wrapper slim plan） | **Done** |

铁律摘要：停增 AppState 业务方法；Write 永不进 ReAct ToolCatalog；ReAct 只走 `dispatch_tool`；Solo L1。

---

## 7. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-07-09 | Wave 0–6 / TN-2 / W1–W6 / R1–R3 |
| 2026-07-09 | **TN-3 拍板**：日常 A、真 AI A、真界面 A、性能 A、结构 S4、测试先量再砍 |
| 2026-07-09 | **TN-3 完成**：P2 UserProfile；P5 入口脚本 + inventory + dedup + L1 bench + 巨石部分拆分 |
| 2026-07-10 | 挂接 **Product App 架构迁移计划**；TN 关后下一主线 = composition root + 产品 App |
| 2026-07-10 | **Product App W0–W6 Phase A**：product_apps + Bound 拆除（非终态） |
| 2026-07-10 | TN review FAIL → 编排 [`PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md) |
| 2026-07-10 | **Product App TN R0–R5 Done** |
| 2026-07-10 | TN Conditional Approve 残留 → 编排 [`PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md`](./PRODUCT_APP_TN_WRAPPER_SLIM_PLAN_2026-07-10.md) |
| 2026-07-10 | **Product App TN Phase C S0–S5 Done** |
