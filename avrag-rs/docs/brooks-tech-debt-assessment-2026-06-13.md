# Brooks-Lint Review — 技术债深度评估

**Mode:** Tech Debt Assessment
**Scope:** `avrag-rs`（34 workspace crate）+ `frontend_next` + `contracts` + `desktop`（全项目深度探测 v5；方法级 + 依赖图 + 接缝复测）
**Health Score:** 61/100
**Trend:** 59 → **61**（+2 vs v4）

**一句话结论：** v4 路线图大部分已兑现——前端 HTTP 统一、桌面 IPC 契约对齐、记忆画像拆分、LLM 客户端拆分、`storage-pg` 直连清零、`dispatch_skill_tool` 从 475 行降至 170 行；剩余债务高度集中在 **ReAct `run()` 主循环（381 行 / 52 行深嵌套）** 与 **app-bootstrap 三个 850+ 行 PG 适配器**。

> **归档：**
> - v1 → [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health 34）
> - v2 → [`archive/brooks-tech-debt-assessment-2026-06-12-v2.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v2.md)（Health 58）
> - v3 → [`archive/brooks-tech-debt-assessment-2026-06-12-v3.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v3.md)（Health 70）
> - v4 → [`archive/brooks-tech-debt-assessment-2026-06-12-v4.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v4.md)（Health 59）

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| Workspace | 34 crate；`cargo metadata` DFS **循环依赖：0** |
| 六类衰减风险 | 全部启用（无 `.brooks-lint.yaml`） |
| 优先级公式 | Pain × Spread（各 1–3，最高 9）；7–9 Critical debt / 4–6 Scheduled / 1–3 Monitored |
| 验证方式 | `cargo metadata` 依赖图、impl 方法级行数与 ≥6 层缩进统计、生产路径 `unwrap` 上下文核查、前端 import 链追踪 |

### 1.1 关键指标对比（v4 → v5）

| 指标 | v4 | v5（本次） | 变化 |
|------|-----|------------|------|
| ReAct `run()` | ~465 行 / ge6≈89 | **381 行 / ge6=52** | ✅ 部分提取（`check_iteration_budget_exhausted`、`emit_turn_end_telemetry` 等已拆出） |
| `dispatch_skill_tool()` | ~475 行 | **170 行**（+ `dispatch_codegen` 等子分发） | ✅ 拆分完成 |
| `run_auto_fallback()` | 未单独计量 | **126 行 / ge6=51** | ❌ 新暴露热点 |
| loop 目录总行 | 6060 | **6309**（policy 子树增长） | ⚠️ 净增 249 行 |
| 前端 7× `client.ts` fetch 重复 | 2288 行各自包装 | **1901 行，全部 `import ../http/request`** | ✅ 统一完成 |
| 桌面 IPC 事件 schema | 手写第三份 | **`contracts::ChatEvent` + `parseIpcChatEvent`** | ✅ 单源对齐 |
| `chat_private` | 1122 行 + 8 处生产 unwrap | **`chat_private/` 模块树 515+499 行；生产 0 unwrap** | ✅ 拆分 + typed delta |
| `llm/client.rs` | 1262 行三合一 | **`client/mod.rs` 375 行** + stream_parser/rate_limit/request | ✅ 拆分完成 |
| `storage-pg` 运行时直连 | 4 个 domain crate | **仅 `app-bootstrap` + `avrag-worker`** | ✅ 架构目标达成 |
| `settings-share-messages` shim | 14 行 + 10 调用方 | **文件与引用均 0** | ✅ 清除 |
| `auth_secondary.rs` | 1040 行 | **拆至 `auth/reset.rs` 等**（最大 537 行） | ✅ 按子域拆分 |
| `repository_retrieval.rs` | 1222 行混域 | **458 + 504（lifecycle）** | ✅ 按域拆分 |
| `use-workspace-context-rail.ts` | 750 行 / 39 hooks | **114 行** | ✅ 大幅瘦身 |
| `WorkspaceChatStreamEvent` | 独立 kind 层 | **`= ChatEvent` 类型别名** | ✅ DTO 统一 |
| `pg_share_store.rs` | 未单列 | **1062 行** | ❌ 新热点 |
| `pg_admin_store.rs` | 未单列 | **894 行** | ❌ 新热点 |
| `billing_sql/core_webhooks.rs` | 未单列 | **884 行** | ❌ 新热点 |
| 生产 TODO/FIXME（Rust） | 3 处 | **3 处**（eval + guardrails） | ✅ 维持 |

---

## 2. Findings

### 🔴 Critical

**Cognitive Overload — ReAct `run()` 仍占 381 行，52 行处于 ≥6 层缩进**

Symptom: `agents/loop/mod.rs` 的 `run()` 占 **L126–506（381 行）**，其中 **52 行缩进 ≥6 层**；虽已提取 `check_iteration_budget_exhausted`、`emit_turn_end_telemetry`、`trigger_auto_fallback_and_check_degraded` 等辅助方法，主循环体仍是 loop-within-match-within-if 的深嵌套控制流。同文件 `build_run_result` 另占 169 行。loop 目录合计 **6309 行**，为全项目最大行为子系统。
Source: Fowler — *Refactoring*, Long Method; McConnell — *Code Complete*, Ch. 7: High-Quality Routines; Ousterhout — *A Philosophy of Software Design*, Deep vs Shallow Modules
Consequence: 所有 agent 行为的主执行路径——调整 exit policy、disclosure、synthesis gate、auto-fallback 时仍需在 380 行方法内定位上下文；修改回归风险集中；新人无法在工作记忆内装下完整控制流。v4→v5 虽有进展，但按 Severity Guide（>50 行且嵌套 >5）仍属 Critical。
Remedy: 继续不动行为地提取：`run()` 内每轮迭代的「LLM 调用 → 解析 → 分发 → 状态更新」四段各成 <80 行私有方法；将 `build_run_result` 移入 `assembler.rs` 或独立 `run_result.rs`。现有 mod.rs/iteration.rs 内嵌 20+ 用例是安全网，拆一步跑一次。
Priority: Pain 3 × Spread 2 = **6**（Scheduled） | Intent: **[accidental]**

---

### 🟡 Warning

**Cognitive Overload — `run_auto_fallback()` 126 行、51 行深嵌套**

Symptom: `agents/loop/mod.rs` 的 `run_auto_fallback()` 占 **L700–825（126 行）**，**51 行缩进 ≥6 层**——与 `run()` 同级的 fallback 路径复杂度被 v4 拆分后暴露出来。
Source: Fowler — *Refactoring*, Long Method; Brooks — *The Mythical Man-Month*, Second-System Effect（局部战术补丁未同步简化）
Consequence: auto-fallback 是 degraded-run 恢复的关键路径；深嵌套使 fallback 分支难以独立测试，与 `run()` 形成「双核心」认知负担。
Remedy: 将 fallback 按触发原因（no_evidence / budget_exhausted / synthesis_blocked）拆为三个 <50 行策略函数，由 `run_auto_fallback` 做薄分发。
Priority: Pain 2 × Spread 2 = **4**（Scheduled） | Intent: **[accidental]**

**Cognitive Overload — app-bootstrap 三个 850+ 行 PG 适配器**

Symptom: `pg_share_store.rs` **1062 行**、`pg_admin_store.rs` **894 行**、`billing_sql/core_webhooks.rs` **884 行**；每个文件混合 RLS `set_config`、事务开启、多表 SQL 与 port trait 实现。`pg_share_store` 单独含 20+ 处 `sqlx::query` 调用。
Source: Fowler — *Refactoring*, Divergent Change; Martin — *Clean Architecture*, SRP
Consequence: share/admin/billing 任一 schema 变更需在同一巨型文件中定位；merge conflict 面大；bootstrap 层 fan-out 达 20，成为 composition root 的认知瓶颈。
Remedy: 按 port 子域再拆——如 `pg_share_store/{tokens,members,analytics}.rs`；抽取共享 `set_rls_context()` 到 `adapters/pg_session.rs`（6 个适配器重复 `set_config` 模式）。参照 `repository_retrieval` 按域拆分先例。
Priority: Pain 2 × Spread 3 = **6**（Scheduled） | Intent: **[accidental]**

**Cognitive Overload — unified agent 工具层 `atomic_tools` + `helpers` 各 860+ 行**

Symptom: `agents/unified/atomic_tools.rs` **869 行**、`helpers.rs` **861 行**；混合 codegen 桥接、检索 citation 构建、observation 序列化、tool result 格式化等多个变更原因。
Source: Fowler — *Refactoring*, Divergent Change / Feature Envy
Consequence: 新增 atomic tool 或调整 codegen observation 格式时，需在 1700 行 unified 层中导航；与 loop/iteration 的 dispatch 子函数形成平行复杂度。
Remedy: 按工具族拆分：`atomic_tools/{retrieval,codegen,search}.rs`；`helpers` 中 codegen 相关移入 `codegen_bridge.rs`（与 iteration 的 `dispatch_codegen` 对齐）。
Priority: Pain 2 × Spread 2 = **4**（Scheduled） | Intent: **[accidental]**

**Knowledge Duplication — 前端 `decodeError` 仍有 4 份拷贝**

Symptom: `lib/http/request.ts` 已有 `decodeApiError`，但 `workspace/stream.ts`、`billing/api.ts`、`dashboard/preferences.ts` 仍各自实现 **几乎相同的 JSON 错误解析**（各 15–25 行）；`billing/api.ts` 另从 `auth/client` 导入 `buildApiUrl` 而非 `http/request`。
Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY
Consequence: 错误 envelope 格式变更需改 4 处；auth 与 http 模块边界模糊，新开发者不知以哪份为准。
Remedy: `stream.ts`/`billing/api.ts`/`preferences.ts` 统一 `import { decodeApiError, buildApiUrl } from "../http/request"`；删除本地 `decodeError`。
Priority: Pain 1 × Spread 3 = **3**（Monitored，边界 Warning） | Intent: **[accidental]**

---

### 🟢 Suggestion

**Change Propagation — `stream.ts` 仍含 ~200 行手写 wire 事件窄化**

Symptom: `WorkspaceChatStreamEvent` 已 alias 为 `ChatEvent`，但 `parseWireChatEvent`/`parseIpcChatEvent` 仍 **419 行**，含 citation/source_locator 手工窄化与 `CHAT_EVENT_NAMES` 校验。
Source: Ousterhout — *A Philosophy of Software Design*, Information Leakage
Consequence: 协议字段变更时映射层可能 drift；风险已因 DTO 统一而显著降低。
Remedy: 评估 generated schema 校验（zod/typia）替代手写 switch；或让 reducer 直接消费 `ChatEvent.event` 删除中间层。
Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[intentional]** UI 适配层

**Cognitive Overload — `loop/policy/config.rs` 669 行 YAML 策略配置**

Symptom: 单文件承载三模式（rag/search/chat）的 YAML 加载、校验、默认值与 10+ 内嵌测试。
Source: McConnell — *Code Complete*, Ch. 7
Consequence: 新增 agent 模式或调整 policy cluster 时需编辑大文件；与 loop 主循环共享变更面。
Remedy: 按 mode 拆 `config/{rag,search,chat}.yaml` + 薄加载器；或提取 `ModeConfig` 校验到独立 `config/validate.rs`。
Priority: Pain 1 × Spread 2 = **2**（Monitored） | Intent: **[accidental]**

**Domain Model Distortion — 记忆画像 typed delta 与 JSON 存储桥接**

Symptom: `profile_merge.rs`（499 行）已引入 `ProfileDelta`/`SlotUpdate` 等 typed struct，但运行时 profile 仍以 `serde_json::Value` 存储与合并（`ensure_profile_object`、`slot_update_to_value` 等桥接函数）。
Source: Evans — *Domain-Driven Design*, Anemic Domain Model; Fowler — *Refactoring*, Primitive Obsession
Consequence: 类型安全仅覆盖 delta 入口，profile 主体仍是无形状 JSON；未来新增 slot 类型需同时维护 struct 与 JSON 路径。
Remedy: 定义 `UserProfile` struct（serde 序列化），merge 函数操作 typed struct 后一次性 `to_value`；与 v4 路线图 #4 方向一致，现可降级为收尾项。
Priority: Pain 1 × Spread 1 = **1**（Monitored） | Intent: **[intentional]**（渐进迁移中）

**Accidental Complexity — eval framework 1633 行（feature 门控）**

Symptom: `eval/framework.rs` 占 1633 行，门控于 `#![cfg(feature = "eval")]`，不进入生产二进制。
Source: Brooks — *The Mythical Man-Month*, Second-System Effect
Consequence: 无生产风险；开启 eval feature 时编译与认知成本高。
Remedy: 按 eval 场景（rag/search/chat/redteam）拆子模块；维持 feature gate。
Priority: Pain 1 × Spread 1 = **1**（Monitored） | Intent: **[intentional]**

---

## 3. Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 5 | 3.4 | Scheduled | accidental |
| Knowledge Duplication | 1 | 3.0 | Monitored | accidental |
| Change Propagation | 1 | 2.0 | Monitored | intentional |
| Domain Model Distortion | 1 | 1.0 | Monitored | intentional |
| Accidental Complexity | 1 | 1.0 | Monitored | intentional |
| Dependency Disorder | 0 | — | **Clean**（无环；storage-pg 直连清零） | — |

**Recommended focus:** Cognitive Overload（ReAct `run()` + `run_auto_fallback` 收尾拆分 → app-bootstrap PG 适配器按域拆分）

**系统性判读:** v4 的 11 项偿还清单中 **7 项已完全清零、2 项大幅进展**；债务重心从「多热点分散」收敛为「两个主堡垒」——agent loop 主路径与 bootstrap PG 适配层。六风险中 Dependency Disorder 首次清零，是本轮最显著的架构收益。

---

## 4. 偿还路线图（v5 更新）

### 4.0 自 v4 以来已完成 ✅

| 任务 | 验收 |
|------|------|
| 前端统一 `lib/http/request.ts` | 8 个 client 全部 import |
| 桌面 IPC `ChatEvent` 对齐 | `desktop/lib.rs` emit contracts；`tauri-ipc.ts` 用 `parseIpcChatEvent` |
| `chat_private` 拆分 + 生产 unwrap 清零 | `chat_private/{mod,profile_merge,profile_types,...}` |
| `llm/client` 拆文件 | `client/{mod,stream_parser,rate_limit,request}.rs` |
| `storage-pg` 域 crate 直连清零 | normal 依赖仅 bootstrap + worker |
| `dispatch_skill_tool` 拆分 | 170 行 + 子分发函数 |
| `settings-share-messages` 删除 | 0 引用 |
| `auth_secondary` 按子域拆分 | `auth/reset.rs` 等 |
| `repository_retrieval` 按域拆分 | retrieval + lifecycle |
| `use-workspace-context-rail` 瘦身 | 114 行 |

### 4.1 计分模型：61 → 100

| 档位 | 数量 | 单项分值 | 合计 | 清完后累计 |
|------|------|---------|------|-----------|
| 🔴 Critical | 1 | +15 | +15 | 61 → **76** |
| 🟡 Warning | 4 | +5 | +20 | 76 → **96** |
| 🟢 Suggestion | 4 | +1 | +4 | 96 → **100** |

### 4.2 第一档 — Critical（61 → 76）

| # | 任务 | 验收 |
|---|------|------|
| 1 | 继续拆 `run()` + `build_run_result` | 单方法 <150 行；ge6 行数 <20；`cargo test -p app-chat` 全绿 |

### 4.3 第二档 — Warning（76 → 96）

| # | 任务 | 验收 |
|---|------|------|
| 2 | 拆 `run_auto_fallback` 三策略 | 单函数 <60 行 |
| 3 | bootstrap PG 适配器按域拆分 | 单文件 <500 行；共享 `set_rls_context` |
| 4 | unified `atomic_tools`/`helpers` 按工具族拆分 | 单文件 <500 行 |
| 5 | 前端 `decodeError` 统一到 `http/request` | 4 处 → 1 处 |

### 4.4 第三档 — Suggestion（96 → 100）

| # | 任务 | 验收 |
|---|------|------|
| 6 | 评估删除 `stream.ts` 手写窄化层 | 或减少至 <200 行 |
| 7 | `loop/policy/config` 按 mode 拆分 | 单文件 <400 行 |
| 8 | `UserProfile` typed struct 收尾 | merge 不再直接操纵 Value |
| 9 | eval framework 按场景拆模块 | 维持 feature gate |

### 4.5 建议执行顺序

```
快赢:      #5 decodeError 统一          (半天)
并行攻坚:  #1 run() 拆分  +  #2 auto_fallback  (每步跑测试)
结构性:    #3 bootstrap 适配器  +  #4 unified 工具层
收尾:      #6 → #7 → #8 → #9
```

### 4.6 维持机制

- 新 PG 适配器文件 >500 行、新函数 >150 行时在 PR review 拦截
- 桌面端新 IPC 命令必须序列化 contracts 类型（已有成例）
- 每完成一批跑集成门禁并重测 Brooks-Lint

### 集成门禁

```bash
cd avrag-rs && cargo test -p app-chat -p app-bootstrap -p avrag-llm
cd contracts && cargo test
cd frontend_next && pnpm check:contracts-drift && pnpm typecheck
./scripts/check_contract_governance.sh
```

---

## 5. 预期成果

| 维度 | v5 | 第一档完成 | 第一+二档完成 | 全部清零 |
|------|-----|-----------|--------------|---------|
| Health Score | 61 | **76** | **96** | **100** |
| Critical | 1 | 0 | 0 | 0 |
| Warning | 4 | 4 | 0 | 0 |
| Suggestion | 4 | 4 | 4 | 0 |
| 最大单方法 | 381 行 (`run`) | <150 行 | <150 行 | <150 行 |
| storage-pg 域直连 | 0 | 0 | 0 | 0 |
| 前端 decodeError 拷贝 | 4 | 4 | 1 | 1 |

---

## 6. 附录：关键文件索引

| 路径 | 行数 | 说明 |
|------|------|------|
| `app-chat/src/agents/loop/mod.rs` | 1201（`run` 381 / `run_auto_fallback` 126） | 🔴 主病灶 |
| `app-bootstrap/src/adapters/pg_share_store.rs` | 1062 | 🟡 巨型适配器 |
| `app-bootstrap/src/adapters/pg_admin_store.rs` | 894 | 🟡 巨型适配器 |
| `app-bootstrap/src/adapters/billing_sql/core_webhooks.rs` | 884 | 🟡 巨型适配器 |
| `app-chat/src/agents/unified/atomic_tools.rs` | 869 | 🟡 工具层 |
| `app-chat/src/agents/unified/helpers.rs` | 861 | 🟡 工具层 |
| `app-chat/src/agents/loop/iteration.rs` | 1147（`dispatch_skill_tool` 170） | ✅ 已拆分 |
| `frontend_next/lib/http/request.ts` | 147 | ✅ HTTP 统一入口 |
| `frontend_next/lib/workspace/stream.ts` | 419 | 🟢 wire 窄化残留 |
| `app-chat/src/chat_private/profile_merge.rs` | 499 | 🟢 typed delta 桥接 |
| `llm/src/client/mod.rs` | 375 | ✅ 已拆分 |
| `desktop/src-tauri/src/lib.rs` | ~320 | ✅ contracts ChatEvent |

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-13 v5（深入探测，方法级口径）*
