# Brooks-Lint Review — 技术债深度评估

**Mode:** Tech Debt Assessment  
**Scope:** `avrag-rs`（34 workspace crate）+ `frontend_next` + `contracts`（全项目深度探测 v2）  
**Health Score:** 58/100  
**Trend:** 34 → 58（+24）— 较 v1 评估显著改善；Phase 1 路线图约 70% 已落地

**一句话结论：** 项目已进入架构清理的收获期——worker 巨型文件、前后端 DTO 手工同步、计费词汇分裂三项 critical 债务已基本解决；当前最大残留是 **SSE 解析层与 codegen 脱节**、**worker 债务向 `pipeline/helpers.rs` 转移**、以及 **`transport-http → app` 门面未拆除**。

> **归档说明：** 上一版报告已移至 [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health Score 34/100，基于未完成的拆分状态）。

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| 源文件规模 | ~4522 个 Rust/TS/TSX 源文件（含测试） |
| Workspace | 34 crate（`crates/` 32 + `bins/` 3） |
| 六类衰减风险 | Cognitive Overload、Change Propagation、Knowledge Duplication、Accidental Complexity、Dependency Disorder、Domain Model Distortion |
| 优先级公式 | Pain × Spread（1–3 各维度，最高 9） |
| 债务意图 | `[intentional]` 有文档/计划；`[accidental]` 无可见偿还计划 |
| 配置 | 无 `.brooks-lint.yaml`，全风险项启用 |

### 1.1 关键指标对比（v1 → v2）

| 指标 | v1（早） | v2（本次） | 变化 |
|------|----------|------------|------|
| `worker/main.rs` | **3263** 行 | **4** 行 | ✅ −99.9% |
| `transport-http/handlers.rs` | 单体 **1834** 行 | 5 文件共 **1771** 行 | ✅ 按域拆分 |
| 前后端契约 | 手写 `stream.ts` DTO | **typeshare + ts-rs** + golden fixtures | ✅ 管道建立 |
| `settings-surface.tsx` | **1572** 行 | **25** 行 | ✅ |
| `messages.ts` | **2519** 行单文件 | **7** 行入口 + **15** 域分片 | ✅ |
| `chat-message-list.tsx` | **1406** 行 | **331** 行 | ✅ |
| E2E `test_context.rs` | 单文件 **1486** 行 | 5 文件，最大 **548** 行 | ✅ |
| `UserTier::Enterprise` | 独立枚举 | **`BillingTier`** + `enterprise→Plus` 归一化 | ✅ |
| Agent `"general"` API 别名 | 保留 | **`parse("general") → None`** | ✅ |
| 最大生产热点 | `worker/main.rs` | `mineru.rs` **1886** / `helpers.rs` **1273** | 债务转移 |
| 运行时循环依赖 | 无 | **无** | 维持健康 |

---

## 2. 积极信号（v1 以来已偿还）

| 区域 | 状态 | 证据 |
|------|------|------|
| Worker 模块化 | ✅ 完成 | `main.rs` 仅调用 `avrag_worker::run()`；逻辑在 `pipeline/`、`indexing/`、`pdf/` 等 20+ 模块 |
| HTTP handlers 拆分 | ✅ 完成 | `handlers/{chat,documents,notebooks,notebook_analysis}.rs` + `mod.rs`（55 行） |
| contracts → TS codegen | ✅ 建立 | `pnpm generate:contracts` → `lib/contracts/generated/`；`golden-fixtures.test.ts` |
| 前端巨型 settings/chat-pane | ✅ 完成 | settings 25 行；workspace-chat-pane 180 行 |
| i18n 域化 | ✅ 完成 | `lib/i18n/messages/*.ts` 15 个分片，合计 ~2630 行 |
| 计费 tier 统一 | ✅ 完成 | `billing/tier.rs`: `Free \| Plus \| Pro`；legacy alias 归一化 |
| eval/redteam | ✅ feature-gate | `app-chat` 的 `eval` feature 默认关闭 |
| 循环依赖 | ✅ 无 | 仅 dev-dep 测试环（`app↔transport-http`） |

---

## 3. Findings

### 🔴 Critical

*（v1 的两项 Critical 已降级或解决；当前无新增 Critical 级 finding。）*

---

### 🟡 Warning

#### Change Propagation — SSE 事件类型双轨维护（codegen 未接入解析层）

Symptom: `contracts` 已通过 ts-rs 生成 `ChatEvent`（`lib/contracts/generated/chat_event.ts`，使用 `"event"` 标签）；`frontend_next/lib/workspace/stream.ts` 仍定义 **92 行**手写 `WorkspaceChatStreamEvent`（使用 `kind` 标签）+ **115 行** `decodeChatEvent` + **28 行** `parseCitation`。DTO 已从 contracts 导入，但 SSE discriminated union 仍独立维护。协议字段变更需同步 contracts codegen **和** stream 解析器。

Source: Winters et al. — *Software Engineering at Google*, Hyrum's Law; Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: chat 协议演进时，TypeScript 编译期无法捕获 SSE 层 drift；流式解析和 citation 渲染路径仍是回归热点。

Remedy: 让 `stream.ts` 消费 generated `ChatEvent`（thin adapter 做 `event`→`kind` 映射或直接统一字段名）；删除 `WorkspaceChatStreamEvent`；扩展 golden fixture 覆盖 SSE decode 路径。

Priority: Pain 3 × Spread 3 = **9** | Intent: **[accidental]**

---

#### Change Propagation — `transport-http` 仍经 `app::AppState` 门面（12 处）

Symptom: `transport-http/src/` 中 **12 个文件** `use app::AppState`（routes + handlers + middleware）；仅 `handlers/chat.rs` 一处直接调 `app_chat::`。`transport-http/Cargo.toml` 同时依赖 `app` + `app-chat` + `app-core`。

Source: Brooks — *The Mythical Man-Month*, Ch. 2 Brooks's Law; Martin — *Clean Architecture*, DIP

Consequence: agent 模块改动仍需穿透 `app` delegate 层；transport 与 app 拆分进度绑定，变更半径被放大。

Remedy: `transport-http` 改依赖 `app-bootstrap::AppState` 或 trait port；消除 12 处 `use app::AppState`。

Priority: Pain 2 × Spread 3 = **6** | Intent: **[intentional]**（过渡期，无 deadline → 按 accidental 计优先级）

---

#### Change Propagation — `common` 高 fan-in（22 crate / 160 文件 import）

Symptom: `common` 被 **22** 个 crate 依赖；**160** 个 `.rs` 文件含 `use common::`；`common/src/lib.rs` 大量 `pub use contracts::...` 并追加 `rag_execute`（626 行）、`tool_call`（605 行）等内部类型。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY; Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 内部类型变更波及全 workspace；新人不清楚新类型应放 `contracts` 还是 `common`。

Remedy: 明确分层：`contracts` = HTTP/API 边界；`common` = 仅 runtime 内部类型；移除 contracts re-export 或合并为单 crate 加 `api`/`internal` module。

Priority: Pain 2 × Spread 3 = **6** | Intent: **[accidental]**

---

#### Cognitive Overload — `pipeline/helpers.rs` 1273 行 ingestion 编排器

Symptom: `bins/worker/src/pipeline/helpers.rs` **1273 行**；import 横跨 **20+** crate（ingestion、storage、retrieval、llm 等）；`document_pipeline.rs` 另 **664** 行。Worker 债务从 `main.rs` **转移**而非消除。

Source: Fowler — *Refactoring*, Long Method; McConnell — *Code Complete*, Ch. 7

Consequence: ingestion/索引逻辑改动仍需在 1200+ 行文件中定位；E2E worker 超时难以隔离根因。

Remedy: 按阶段拆为 `parse/`、`index/`、`notify/` 子模块；单文件目标 ≤400 行。

Priority: Pain 3 × Spread 2 = **6** | Intent: **[accidental]**

---

#### Cognitive Overload — ReAct Agent Loop ~6008 行 / 17 模块

Symptom: `app-chat/src/agents/loop/` 共 **6008 行**（`mod.rs` **1088**、`iteration.rs` **1009**、`config.rs` **669**）；`ReActLoop` 持有多个 `Option<Arc<...>>` 依赖。

Source: Ousterhout — *A Philosophy of Software Design*, Deep vs Shallow Modules

Consequence: 修改 disclosure phase、exit policy 或 synthesis gate 需跨多个子模块理解。

Remedy: 绘制 loop 状态机单页文档；将 `config`/`exit_policy`/`disclosure_plan` 合并为 `LoopPolicy` 深模块，对外只暴露 `run(request) -> AgentRunResult`。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**（部分来自 domain 固有复杂度）

---

#### Cognitive Overload — 前端巨型组件与 hook 未拆分

Symptom: `workspace-share-surface.tsx` **1319** 行、`dashboard-surface.tsx` **1057** 行、`workspace-right-rail.tsx` **930** 行；`use-chat-session.ts` **912** 行（SSE 状态机 + 打字机效果 + 9 种 event handler）。

Source: Fowler — *Refactoring*, Long Method

Consequence: 分享/仪表盘/右侧栏改动在 900–1300 行文件中 merge conflict；chat 流式逻辑集中在单一 hook。

Remedy: share/dashboard/right-rail 按 Tab/Panel 拆分；`use-chat-session` 提取 event reducer 子 hook。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**

---

#### Knowledge Duplication — UI `ChatMessage` 与 contract 同名不同义

Symptom: `hooks/use-chat-session.ts` 定义 UI 态 `ChatMessage`（`id: string`、含 `mode`/`pending`/`guarded`）；contract `ChatMessage` 为 wire format（不同字段集）。同名导致 import 混淆和搜索误导。

Source: Evans — *Domain-Driven Design*, Ubiquitous Language; Fowler — *Refactoring*, Alternative Classes with Different Interfaces

Consequence: 新开发者混淆两种 `ChatMessage`；重构时易改错类型。

Remedy: 重命名 UI 类型为 `UiChatMessage`；wire 类型统一从 `../contracts` 导入。

Priority: Pain 3 × Spread 3 = **9** | Intent: **[accidental]**（命名遗留）

---

#### Dependency Disorder — Domain crate 直连 storage（5 条）

Symptom: 正常运行时依赖：`app → avrag-storage-pg`、`app-admin → avrag-storage-pg`、`app-bootstrap → avrag-storage-pg + avrag-storage-milvus`、`app-documents → avrag-storage-pg`。`rag-core` 无 storage 直连（✅）。

Source: Martin — *Clean Architecture*, DIP

Consequence: 分层可测试性削弱；storage 驱动变更可能波及 domain crate。

Remedy: 经 repository trait/port 注入；composition root（`app-bootstrap`）保留唯一 concrete wiring。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[intentional]** composition root 模式

---

### 🟢 Suggestion

#### Cognitive Overload — `handlers/notebooks.rs` 924 行

Symptom: handlers 拆分后 notebooks 仍为单文件 **924** 行；对比 `chat.rs` **474**、`documents.rs` **170**。

Remedy: 按 notebook CRUD / sessions / notes 再拆。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**

---

#### Accidental Complexity — `eval_framework.rs` 1633 行（feature-gate 但未剥离）

Symptom: `app-chat/src/agents/eval_framework.rs` **1633 行**；`eval` feature 默认关闭，但文件仍在主 crate 树中。

Remedy: 移入独立 crate 或 `tests/`；或保持 feature-gate 并文档化。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[intentional?]**

---

#### Accidental Complexity — 散布 `#[allow(dead_code)]`（24+ 处）

Symptom: `transport-http/lib_impl/infra_handlers.rs`（5 处）、`auth_types.rs`（8 处）、`rag_prompts.rs`（2 处）等。

Remedy: 逐文件清理或替换为 `#[cfg(test)]`。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Accidental Complexity — `rag_prompts.rs` 1739 行 prompt 字典

Symptom: 全部 RAG prompt 模板集中于单文件；含 dead_code 标记的未使用模板。

Remedy: 外置 YAML/模板文件；按 agent/mode 分目录。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Knowledge Duplication — `settings-share-messages.ts` 725 行平行 i18n

Symptom: 复制 `UiMessageDescriptor` 模式，与 `messages/settings.ts`、`messages/usage.ts` 键重叠。

Remedy: 合并进 `lib/i18n/messages/` 域分片。

Priority: Pain 2 × Spread 3 = **6** → 降为 Suggestion（非阻断） | Intent: **[accidental]**

---

#### Accidental Complexity — codegen 未接入 build/CI

Symptom: `pnpm generate:contracts` 存在，但无 `prebuild`/`postinstall` 钩子；CI workflow 未调用；生成物提交 git 但无 drift check。

Remedy: CI 加 `pnpm generate:contracts && git diff --exit-code lib/contracts/generated/`。

Priority: Pain 2 × Spread 3 = **6** → Suggestion | Intent: **[accidental]**

---

#### Knowledge Duplication — Plus 用量倍数文案 6× vs 10× 不一致

Symptom: `messages/paywall.ts` 写 **10×**；`messages/usage.ts` 的 `usageUpgradeCta` 写 **6×**（同文件其他键写 10×）。

Remedy: 产品确认后统一为单一倍数；更新测试断言。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Domain Model Distortion — `enterprise` DB 别名与 `UserTier` 过渡别名

Symptom: 应用逻辑已统一 `BillingTier::Free/Plus/Pro`；`enterprise` 仅作 DB migration 兼容（`tier.rs:37` 归一化、`admin-i18n.ts` 显示映射）；`react_loop.rs:27` 仍 `pub use BillingTier as UserTier`。

Remedy: 逐步移除 `UserTier` 别名；DB alias 保留至 migration 完成。

Priority: Pain 1 × Spread 1 = **1** | Intent: **[intentional]**

---

#### Knowledge Duplication — `"general"` 域标签残留（非 API 别名）

Symptom: `AgentKind::parse("general")` 已返回 `None`；但 `memory_helpers.rs`、`i18n.rs`、`storage-pg/utility.rs` 仍用 `"general"` 作域/显示标签。

Remedy: 统一为 `"chat"` 或文档化 legacy 显示用途。

Priority: Pain 1 × Spread 1 = **1** | Intent: **[intentional]**

---

## 4. Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 4 | 5.0 | Scheduled | accidental |
| Change Propagation | 3 | 7.0 | **Critical 残留** | mixed |
| Knowledge Duplication | 3 | 3.3 | Monitored | mixed |
| Accidental Complexity | 4 | 2.0 | Monitored | mixed |
| Dependency Disorder | 1 | 4.0 | Scheduled | intentional |
| Domain Model Distortion | 1 | 1.0 | **Resolved** | intentional |

**Recommended focus:** Change Propagation（SSE 层接入 codegen + 消除 `app::AppState` 门面）→ Cognitive Overload（`pipeline/helpers.rs` 二次拆分 + 前端巨型 surface）

---

## 5. 偿还路线图（基于当前状态）

### Phase 1 — 立即可做（v1 多项已完成 ✅）

| 状态 | 任务 | 验收 |
|------|------|------|
| ✅ | worker `main.rs` 拆分 | `main.rs` = 4 行 |
| ✅ | contracts → TS codegen | `pnpm generate:contracts` + golden fixtures |
| ✅ | billing tier 统一 | `BillingTier` + `enterprise→Plus` |
| ✅ | handlers 按域拆分 | 5 文件（notebooks 待再拆） |
| ✅ | 前端 settings/chat-pane/messages 拆分 | 见 §2 |
| ⬜ | **SSE 层接入 `ChatEvent`** | 删除 `WorkspaceChatStreamEvent` |
| ⬜ | **`pipeline/helpers.rs` 按阶段拆分** | 单文件 <400 行 |
| ⬜ | **CI codegen drift gate** | `generate:contracts` + diff check |

### Phase 2 — 消除变更传播

| 任务 | 验收 |
|------|------|
| `transport-http` 改依赖 `app-bootstrap::AppState` | 0 处 `use app::AppState` |
| 重命名 UI `ChatMessage` → `UiChatMessage` | typecheck 通过 |
| `common` 分层：contracts re-export 移出 | import 路径清晰化 |
| `notebooks.rs` 再拆 | 单文件 <500 行 |

### Phase 3 — 后续

- 拆分 top 3 前端巨型 surface（share / dashboard / right-rail）
- 合并 `settings-share-messages.ts` 进 i18n 域分片
- `eval_framework` 移入独立 crate
- `rag_prompts.rs` 外置模板
- 扩展 `contracts/tests/` 至 billing/notebooks

### 集成门禁（每 Phase 结束）

```bash
# Rust
cd avrag-rs && cargo test -p contracts -p app -p app-chat -p transport-http -p avrag-billing

# Frontend
cd frontend_next && pnpm generate:contracts && pnpm typecheck && pnpm test tests/contracts/

# Governance
./scripts/check_contract_governance.sh
```

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| SSE `ChatEvent` 字段名（`event` vs `kind`）不兼容 | thin adapter 或统一 wire format 后再删手写 union |
| `helpers.rs` 拆分引入 subtle 行为变化 | 纯 move-refactor；跑 product_e2e smoke |
| transport 改 app-bootstrap 与 S3 拆分冲突 | 严格顺序：app 迁移收尾 → transport 改 import |
| Plus 倍数文案统一需产品确认 | 先查 pricing 页面权威值，再改 i18n |

---

## 7. 预期成果

| 维度 | v1 | Phase 1–2 完成后预期 |
|------|-----|----------------------|
| Health Score | 34 | **65–72** |
| Change Propagation | 2 Critical | 0 Critical, ≤2 Warning |
| Cognitive Overload | 2 Critical | 0 Critical, ≤3 Warning |
| 前端 chat 协议变更 | 6+ 处手工同步 | contracts → regenerate → typecheck |

---

## 8. 附录：关键文件索引

| 路径 | 说明 |
|------|------|
| `contracts/src/chat.rs` | Rust chat 协议权威定义（731 行，32 公开类型） |
| `contracts/tests/chat_json.rs` | JSON 契约测试 + golden fixture 导出 |
| `scripts/generate-contracts.sh` | typeshare + ts-rs 生成管线 |
| `frontend_next/lib/contracts/generated/` | 生成 TS 类型 + fixtures |
| `frontend_next/lib/workspace/stream.ts` | SSE 解析（待接入 `ChatEvent`） |
| `avrag-rs/bins/worker/src/pipeline/helpers.rs` | 1273 行 ingestion 编排（新热点） |
| `avrag-rs/bins/worker/src/main.rs` | 4 行入口（已拆分 ✅） |
| `avrag-rs/crates/transport-http/src/handlers/` | 按域拆分（notebooks 924 行待再拆） |
| `avrag-rs/crates/app-chat/src/agents/loop/` | ReAct loop ~6008 行 |
| `frontend_next/hooks/use-chat-session.ts` | 912 行 chat hook |
| `frontend_next/components/share/workspace-share-surface.tsx` | 1319 行最大前端组件 |
| `.brooks-lint-history.json` | Brooks-Lint 历史分数 |

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-12 v2（深入探测）*
