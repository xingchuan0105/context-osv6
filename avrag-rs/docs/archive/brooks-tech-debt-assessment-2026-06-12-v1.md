# Brooks 技术债评估 — 2026-06-12

Brooks-Lint **Tech Debt Assessment** 对全项目的深度结构性债务审计结论。范围：`avrag-rs`（35 个 workspace crate）、`frontend_next`、`contracts`；含当前未提交的大规模 crate 拆分（196 文件变更，87 删除，约 34k 行删除）。

**Mode:** Tech Debt Assessment  
**Scope:** `avrag-rs` + `frontend_next` + `contracts`（全项目深度审计）  
**Health Score:** 34/100  
**Trend:** Tech Debt Assessment 首次运行；参考 Architecture Audit 73/100（2026-06-10）、Health Dashboard 80/100（2026-06-11）

**一句话结论：** 项目正处于 Phase 5 架构清理的正确方向上（chat-pane 已成功瘦身、agent 逻辑已向 `app-chat` 迁移），但前后端 API 类型手工同步、worker/handlers 巨型文件、以及 app→app-chat 拆分未完成，是当前最大的结构性债务来源。

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| 代码规模 | ~1686 个 Rust/TS/TSX 源文件；`avrag-rs` 最大单文件 `worker/main.rs` 3263 行 |
| 六类衰减风险 | Cognitive Overload、Change Propagation、Knowledge Duplication、Accidental Complexity、Dependency Disorder、Domain Model Distortion |
| 优先级公式 | Pain × Spread（1–3 各维度，最高 9） |
| 债务意图 | `[intentional]` 有文档/计划；`[accidental]` 无可见偿还计划 |
| 配置 | 无 `.brooks-lint.yaml`，全风险项启用 |

### 1.1 与相关审计的关系

| 文档 | 分数 | 侧重 |
|------|------|------|
| 本文 | 34 | 结构性债务、偿还优先级、路线图 |
| [brooks-pr-review-2026-06-12.md](./brooks-pr-review-2026-06-12.md) | 43 | 当前工作区 PR 可合并性、编译/未跟踪文件 |
| [brooks-test-quality-review-2026-06-12.md](./brooks-test-quality-review-2026-06-12.md) | 52 | 测试套件结构质量 |
| [architecture-review-2026-06.md](./architecture-review-2026-06.md) | — | RAG/Chat 策略决策（部分已被 ADR-0007 取代） |
| [t13-app-split-inventory.md](./t13-app-split-inventory.md) | — | app 拆分 crate 职责清单 |

---

## 2. 积极信号（偿还中的进展）

| 区域 | 之前 | 现在 | 状态 |
|------|------|------|------|
| `workspace-chat-pane.tsx` | 2514 行 | 180 行 | 已提取 `use-chat-session` hook |
| `app` crate agent 代码 | 内嵌于 app | 迁移至 `app-chat` | 进行中（87 文件已删） |
| Workspace 35 crate 拆分 | 单 crate | app-*/rag-core/ingestion 等 | 方向正确，未完成 |
| 循环依赖 | — | 无检出 | 健康 |
| Rust 契约测试 | — | `contracts/tests/chat_json.rs`、`transport-http/tests/chat_stream_contract.rs` | 有，但未覆盖 TS |

---

## 3. Findings

### 3.1 Critical

#### Change Propagation — 前后端 API 类型手工同步，无 codegen 管道

| 字段 | 内容 |
|------|------|
| **Symptom** | `contracts/src/chat.rs` 为 Rust 权威定义；`frontend_next/lib/workspace/stream.ts` 手工复制 `AnswerBlock`、`Citation`、`ToolResult`、`ChatTurnInput` 等；全前端无 OpenAPI/TS codegen。修改 SSE 事件或 citation 字段需同步改 contracts → common → transport-http → app-chat → frontend stream/client/hooks/tests，至少 6 处。已有漂移：`ChatRequest.debug`、`GuardReport`、`PlannerOutput` 等 TS 简化版与 Rust 不一致。 |
| **Source** | Winters et al. — *Software Engineering at Google*, Hyrum's Law; Ousterhout — *A Philosophy of Software Design*, Information Leakage |
| **Consequence** | 字段漂移不会在编译期被前端捕获；每次 chat 协议演进都变成跨语言协调工程，回归风险集中在流式解析和 citation 渲染路径。 |
| **Remedy** | 从 `contracts` crate 用 `typeshare`（或 `ts-rs`）生成 TS → `frontend_next/lib/contracts/generated/`；`stream.ts` 只保留 SSE 解析逻辑；从 `contracts/tests/chat_json.rs` 导出 golden JSON fixture 供 TS contract test。 |
| **Priority** | Pain 3 × Spread 3 = **9** |
| **Intent** | [accidental] |

#### Cognitive Overload — `avrag-worker` 主文件 3263 行，多职责巨型编排器

| 字段 | 内容 |
|------|------|
| **Symptom** | `bins/worker/src/main.rs` 单文件 3263 行，仅 7 个 `mod` 声明，ingestion 任务认领、PDF/OCR 路由、Milvus 索引、analytics/audit/orphan jobs 全部内联；import 横跨 20+ crate。 |
| **Source** | Fowler — *Refactoring*, Long Method; McConnell — *Code Complete*, Ch. 7 High-Quality Routines |
| **Consequence** | ingestion 或索引逻辑改动需在 3000+ 行文件中定位；新人 onboarding worker 路径成本极高；E2E worker 超时问题难以隔离根因。 |
| **Remedy** | 按已有 mod 边界拆为 `ingestion/adapters.rs`、`cleanup/mod.rs`、`pipeline/parse.rs`、`indexing/toc.rs` 等；`main.rs` 只保留 wiring + tick loop（目标 ≤200 行）。 |
| **Priority** | Pain 3 × Spread 2 = **6** |
| **Intent** | [accidental] |

---

### 3.2 Warning

#### Change Propagation — app→app-chat crate 拆分进行中，变更半径被放大

| 字段 | 内容 |
|------|------|
| **Symptom** | 工作区 196 文件变更、87 文件删除；`app` 已瘦至 ~2215 行且几乎全是 `pub use` 重导出，但 `transport-http`、`bins/worker` 仍依赖 `app` 门面；`CONTEXT.md` 仍引用 `crates/app/src/agents/` 旧路径。 |
| **Source** | Brooks — *The Mythical Man-Month*, Ch. 2 Brooks's Law; Fowler — *Refactoring*, Shotgun Surgery |
| **Consequence** | 拆分完成前，agent 模块移动需同时更新 facade re-export、调用方 import、文档和 E2E fixture。 |
| **Remedy** | 完成拆分后让 `transport-http`/`worker` 直接依赖 `app-chat`/`app-bootstrap`；删除或限缩 `app` facade；同步更新 `CONTEXT.md`。CONTEXT.md 已有 6 项 agent 清理计划 — 需设定完成 deadline。 |
| **Priority** | Pain 2 × Spread 3 = **6** |
| **Intent** | [intentional]（有文档计划，无可见 ticket/deadline，按 accidental 优先级处理） |

#### Cognitive Overload — `transport-http/handlers.rs` 1834 行单文件承载全部路由

| 字段 | 内容 |
|------|------|
| **Symptom** | notebooks、sessions、documents、chat stream、RAG execute 等所有 HTTP handler 集中在 `crates/transport-http/src/handlers.rs`；与 `routes/*.rs` 中 admin/billing 已 inline 的模式不一致。 |
| **Source** | Fowler — *Refactoring*, Long Method; Martin — *Clean Architecture*, SRP |
| **Consequence** | 新增 API 或修改错误响应时 merge conflict 频繁；各 domain 团队无法并行修改 handler。 |
| **Remedy** | 按 domain 拆为 `handlers/chat.rs`、`handlers/documents.rs`、`handlers/notebooks.rs` 等，与 `routes/` 对齐。 |
| **Priority** | Pain 2 × Spread 3 = **6** |
| **Intent** | [accidental] |

#### Knowledge Duplication — `common` + `contracts` 双层类型体系

| 字段 | 内容 |
|------|------|
| **Symptom** | `contracts` 定义 API DTO；`common` 大量 `pub use contracts::...` 并追加 `rag_execute`、`tool_call` 等内部类型；709 处 import 分散在 179 个 Rust 文件；前端 `stream.ts` 第三层手工复制。`scripts/check_contract_governance.sh` 治理 Rust DTO，**不覆盖** `frontend_next`。 |
| **Source** | Hunt & Thomas — *The Pragmatic Programmer*, DRY; Evans — *Domain-Driven Design*, Bounded Context |
| **Consequence** | 同一概念在三处维护；新人不清楚新类型应放哪一层。 |
| **Remedy** | 明确分层：`contracts` = HTTP/API 边界；`common` = 仅内部 runtime 类型；或合并单 crate 加 `api`/`internal` module。 |
| **Priority** | Pain 2 × Spread 3 = **6** |
| **Intent** | [accidental] |

#### Cognitive Overload — ReAct Agent Loop 子系统 ~6000 行 / 17 模块

| 字段 | 内容 |
|------|------|
| **Symptom** | `app-chat/src/agents/loop/` 含 17 个文件共 ~6005 行；`ReActLoop` 持有 6 个 Option/Arc 依赖。 |
| **Source** | Ousterhout — *A Philosophy of Software Design*, Deep vs Shallow Modules |
| **Consequence** | 修改 disclosure phase、exit policy 或 synthesis gate 需跨多个子模块理解。 |
| **Remedy** | 绘制 loop 状态机单页文档；考虑将 `config`/`exit_policy`/`disclosure_plan` 合并为 `LoopPolicy` 深模块，对外只暴露 `run(request) -> AgentRunResult`。 |
| **Priority** | Pain 2 × Spread 2 = **4** |
| **Intent** | [accidental]（部分复杂度来自 domain 本身） |

#### Domain Model Distortion — 计费层级词汇分裂（Enterprise vs Plus/Pro）

| 字段 | 内容 |
|------|------|
| **Symptom** | `CONTEXT.md` 声明 Plus/Pro 已取代 Enterprise；`app-chat/src/agents/react_loop.rs` 仍定义 `UserTier::Enterprise`；`frontend_next/components/admin/admin-i18n.ts` 仍渲染 "enterprise"/"企业版"；billing migration 0037 测试保留 enterprise unlimited policy。 |
| **Source** | Evans — *Domain-Driven Design*, Ubiquitous Language |
| **Consequence** | 产品说 "Plus" 时工程师在代码里找 `Enterprise`；quota 变更可能漏改 agent budget 或 admin 显示。 |
| **Remedy** | 统一 tier enum 为 `Free | Plus | Pro`（enterprise 仅作 DB migration alias）；react_loop budget 改读 billing policy；admin-i18n 对齐。 |
| **Priority** | Pain 2 × Spread 2 = **4** |
| **Intent** | [accidental] |

#### Cognitive Overload — 前端 UI 巨型组件未拆分

| 字段 | 内容 |
|------|------|
| **Symptom** | `settings-surface.tsx` 1572 行、`chat-message-list.tsx` 1406 行、`dashboard-surface.tsx` 1057 行；`messages.ts` 2519 行单文件 i18n。对比已成功的 `workspace-chat-pane.tsx`（180 行）。 |
| **Source** | Fowler — *Refactoring*, Long Method |
| **Consequence** | settings/billing/profile 改动在 1500 行文件中 merge；citation 渲染与 progress UI 耦合。 |
| **Remedy** | settings 拆为 Tab 组件；chat-message-list 拆为 `CitationRenderer`、`ProgressTimeline`；i18n 按 domain 分文件。 |
| **Priority** | Pain 2 × Spread 2 = **4** |
| **Intent** | [accidental] |

#### Accidental Complexity — eval_framework + redteam 子系统未接入生产路径

| 字段 | 内容 |
|------|------|
| **Symptom** | `eval_framework.rs` 1633 行 + `redteam/` 模块完整存在；`RedTeamService` 仅在 crate 内测试引用，无 worker/api 调度入口。 |
| **Source** | Fowler — *Refactoring*, Speculative Generality |
| **Consequence** | 维护 agent 模块时需理解 redteam 框架，但不影响用户-facing 功能；增加 compile time。 |
| **Remedy** | 移到独立 crate 或 `tests/`；或 feature-gate `#[cfg(feature = "redteam-eval")]`。 |
| **Priority** | Pain 1 × Spread 2 = **2** |
| **Intent** | [intentional?]（无可见 payback plan → 按 accidental 处理） |

#### Change Propagation — E2E `test_context.rs` 1486 行上帝 fixture

| 字段 | 内容 |
|------|------|
| **Symptom** | `crates/app/tests/product_e2e/test_context.rs` 单文件 1486 行；`mock_servers.rs` 1104 行。 |
| **Source** | Meszaros — *xUnit Test Patterns*, General Fixture |
| **Consequence** | 新增 E2E profile 需编辑巨型 fixture；测试失败难以定位 bootstrap vs 业务逻辑。 |
| **Remedy** | 按 profile 拆为 `SmokeContext`、`RagContext`、`LlmRealContext` builder。 |
| **Priority** | Pain 2 × Spread 2 = **4** |
| **Intent** | [accidental] |

#### Cognitive Overload — `messages.ts` 2519 行单文件 i18n 字典

| 字段 | 内容 |
|------|------|
| **Symptom** | 全部 UI 文案集中在 `frontend_next/lib/i18n/messages.ts` 的 `UI_MESSAGES` 对象。 |
| **Source** | McConnell — *Code Complete*, Ch. 11 Variable Names |
| **Consequence** | 并行翻译改动频繁 conflict；难以按 feature 懒加载 locale。 |
| **Remedy** | 拆为 `messages/admin.ts`、`messages/workspace.ts`、`messages/billing.ts`。 |
| **Priority** | Pain 1 × Spread 3 = **3** |
| **Intent** | [accidental] |

---

### 3.3 Suggestion

#### Accidental Complexity — `app` crate 纯 re-export 门面 + crate 级 dead_code 抑制

| 字段 | 内容 |
|------|------|
| **Symptom** | `app/src/lib.rs` 顶部 `#![allow(dead_code)]` / `#![allow(deprecated)]`；44 个源文件几乎全是 `pub use` 转发。 |
| **Remedy** | 迁移完成后删除 `app` crate 或限缩为 composition root；移除 allow 属性。 |
| **Priority** | Pain 1 × Spread 2 = **2** |
| **Intent** | [intentional] |

#### Knowledge Duplication — CONTEXT.md 路径与代码不同步

| 字段 | 内容 |
|------|------|
| **Symptom** | `CONTEXT.md` 仍引用 `crates/app/src/agents/...`；仍提 "2514-line WorkspaceChatPane"（已降至 180 行）。 |
| **Remedy** | 批量更新路径和行数；CI lint 检查 CONTEXT 中的 crate 路径是否存在。 |
| **Priority** | Pain 1 × Spread 2 = **2** |
| **Intent** | [accidental] |

#### Knowledge Duplication — AgentKind "general" 遗留别名

| 字段 | 内容 |
|------|------|
| **Symptom** | `app-chat/src/agents/mod.rs` 保留 `"general"` → `Chat` 别名，供 E2E 和历史 API 客户端。 |
| **Remedy** | API deprecation warning；E2E 改 `"chat"` 后移除别名。 |
| **Priority** | Pain 1 × Spread 1 = **1** |
| **Intent** | [intentional] |

#### Dependency Disorder — `transport-http` 经 `app` 门面访问全部应用层

| 字段 | 内容 |
|------|------|
| **Symptom** | 仅 `transport-http` 直接依赖 `app`；`app` 聚合 8 个内部 crate。 |
| **Remedy** | transport 直接依赖所需 app-* crate 的 trait/port。 |
| **Priority** | Pain 1 × Spread 2 = **2** |
| **Intent** | [accidental] |

---

## 4. Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 5 | 4.0 | Scheduled | accidental |
| Change Propagation | 4 | 6.3 | **Critical** | mixed |
| Knowledge Duplication | 3 | 1.7 | Monitored | mixed |
| Accidental Complexity | 2 | 2.0 | Monitored | mixed |
| Dependency Disorder | 1 | 2.0 | Monitored | accidental |
| Domain Model Distortion | 1 | 4.0 | Scheduled | accidental |

**Recommended focus:** Change Propagation（前后端类型同步 + 未完成 crate 拆分）→ Cognitive Overload（worker/handlers/前端巨型组件）

---

## 5. 偿还路线图（P0 + P1）

以下路线与 Subagent 编排计划对齐，按依赖分阶段执行。

### Phase 1 — 三路并行（无交叉依赖）

| Stream | 任务 | 目录边界 | 验收 |
|--------|------|----------|------|
| **S1** | contracts → TypeScript codegen（typeshare） | `contracts/`、`scripts/`、`frontend_next/lib/contracts/` | `cargo test -p contracts`；`pnpm generate:contracts` |
| **S2** | worker `main.rs` 模块化拆分 | `avrag-rs/bins/worker/src/` | `cargo build -p avrag-worker` |
| **S5** | billing tier 词汇统一（Free/Plus/Pro） | `app-chat/react_loop.rs`、`billing/`、`admin-i18n.ts` | `cargo test -p avrag-billing -p app-chat` |

### Phase 2 — S1 完成后

| Stream | 任务 | 依赖 | 验收 |
|--------|------|------|------|
| **S3** | app crate 迁移收尾（billing_context 修复、legacy 删除、transport 改 app_chat import） | Phase 1 | `cargo test -p app -p transport-http` |
| **S4** | handlers.rs 按 domain 拆分 | S3 先合并 | `chat_stream_contract.rs` 通过 |
| **S6** | 前端接入生成类型 + cross-lang contract test | S1 | `pnpm typecheck`；无重复手写 DTO |

### Phase 3 — 留待后续

- UI 巨型组件拆分（settings、chat-message-list）
- `messages.ts` 按 domain 拆分
- E2E `test_context` 拆分
- redteam feature-gate 或 archive

### 集成门禁（每 Phase 结束）

```bash
# Rust
cd avrag-rs && cargo test -p contracts -p app -p app-chat -p transport-http -p avrag-billing

# Frontend
cd frontend_next && pnpm generate:contracts && pnpm typecheck && pnpm test tests/workspace/

# Governance
./scripts/check_contract_governance.sh
```

**合并顺序建议：** S1 → S5/S2（任意）→ 集成 → S3 → S4 → S6

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| typeshare 不支持 `ChatEvent` tagged enum | fallback：`ts-rs` 或 SSE 变体保留 thin TS wrapper |
| worker 拆分引入 subtle 行为变化 | 纯 move-refactor，不改函数签名；跑 product_e2e smoke |
| S3/S4 同时改 transport-http | 严格顺序：S3 merge → S4 rebase |
| Enterprise 用户 quota 回归 | 保留 DB row + alias 层；migration 0037 test 必须 pass |

---

## 7. 预期成果

- Change Propagation：Critical → Warning
- Cognitive Overload（worker/handlers）：Critical → Warning
- 前端 chat 协议字段变更：改 contracts → regenerate → typecheck 即可捕获 drift
- `app` crate 成为薄 facade，为后续删除 re-export 层铺路

---

## 8. 附录：关键文件索引

| 路径 | 说明 |
|------|------|
| `contracts/src/chat.rs` | Rust chat 协议权威定义 |
| `contracts/tests/chat_json.rs` | JSON 契约测试 |
| `frontend_next/lib/workspace/stream.ts` | 手写 TS 类型 + SSE 解析（待 S6 接入 codegen） |
| `avrag-rs/bins/worker/src/main.rs` | 3263 行 worker 巨型文件 |
| `avrag-rs/crates/transport-http/src/handlers.rs` | 1834 行 HTTP handler 单体 |
| `avrag-rs/crates/app/src/lib.rs` | app facade re-export |
| `avrag-rs/crates/app-chat/src/agents/` | agent 域（已从 app 迁出） |
| `scripts/check_contract_governance.sh` | Rust DTO 治理（不含 frontend_next） |
| `.brooks-lint-history.json` | Brooks-Lint 历史分数 |

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-12*
