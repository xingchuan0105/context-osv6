# Brooks-Lint Review — 技术债深度评估

**Mode:** Tech Debt Assessment  
**Scope:** `avrag-rs`（34 workspace crate）+ `frontend_next` + `contracts`（全项目深度探测 v3）  
**Health Score:** 70/100  
**Trend:** 34 → 58 → **70**（+12 vs v2，+36 vs v1）— Phase 1–2 路线图大部分已落地

**一句话结论：** 自 v2 以来，SSE 已接入 generated `ChatEvent`、`transport-http` 已改依赖 `app-bootstrap`、`pipeline/helpers` 与前端巨型 surface 均已拆分；当前主要残留是 **ReAct loop ~6009 行**、**ingestion 解析器 `mineru.rs` 1886 行**、以及 **`common` 高 fan-in（24 crate）**。

> **归档说明：**
> - v1 → [`archive/brooks-tech-debt-assessment-2026-06-12-v1.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v1.md)（Health 34）
> - v2 → [`archive/brooks-tech-debt-assessment-2026-06-12-v2.md`](./archive/brooks-tech-debt-assessment-2026-06-12-v2.md)（Health 58）

---

## 1. 审计范围与方法

| 维度 | 说明 |
|------|------|
| 源文件规模 | ~4612 个 Rust/TS/TSX 源文件（含测试） |
| Workspace | 34 crate（`crates/` 32 + `bins/` 3） |
| 六类衰减风险 | Cognitive Overload、Change Propagation、Knowledge Duplication、Accidental Complexity、Dependency Disorder、Domain Model Distortion |
| 优先级公式 | Pain × Spread（1–3 各维度，最高 9） |
| 债务意图 | `[intentional]` / `[accidental]` |
| 配置 | 无 `.brooks-lint.yaml`，全风险项启用 |

### 1.1 关键指标对比（v1 → v2 → v3）

| 指标 | v1 | v2 | v3（本次） | 变化 |
|------|-----|-----|------------|------|
| `worker/main.rs` | 3263 | 4 | **4** | ✅ 维持 |
| `pipeline/helpers.rs` | — | 1273 | **255** | ✅ v2→v3 拆分完成 |
| `transport-http → app::AppState` | 12 处 | 12 | **0** | ✅ 改 `app-bootstrap` |
| SSE 类型 | 全手写 DTO | 手写 union | **derived from `ChatEvent`** | ✅ WireToWorkspace |
| `UiChatMessage` 命名 | 与 contract 冲突 | 冲突 | **已重命名** | ✅ |
| `use-chat-session.ts` | 912 | 912 | **95** + `chat-session/` 模块 | ✅ |
| `workspace-share-surface.tsx` | — | 1319 | **100** | ✅ |
| `dashboard-surface.tsx` | 1057 | 1057 | **337** | ✅ |
| `workspace-right-rail.tsx` | 930 | 930 | **41** | ✅ |
| CI codegen drift check | 无 | 无 | **frontend-unit.yml** | ✅ |
| ReAct loop | ~6005 | ~6008 | **6009** | 未变 |
| `common` fan-in | 22 crate | 22 | **24 crate / 163 文件** | 略升 |
| 运行时循环依赖 | 无 | 无 | **无** | ✅ |

---

## 2. 积极信号（v2 以来新偿还）

| 区域 | 状态 | 证据 |
|------|------|------|
| Worker pipeline 二次拆分 | ✅ | `pipeline/helpers.rs` 1273 → **255** 行 |
| transport 门面拆除 | ✅ | `transport-http/Cargo.toml` 依赖 `app-bootstrap`；**0** 处 `use app::AppState` |
| SSE 接入 codegen | ✅ | `stream.ts` import `ChatEvent`；`WireToWorkspace` 从 generated type 派生 `WorkspaceChatStreamEvent` |
| Chat hook 模块化 | ✅ | `hooks/chat-session/{types,helpers,use-chat-stream,use-message-history,use-progress-tracker}.ts` |
| UI 类型命名 | ✅ | `UiChatMessage` 在 `chat-session/types.ts` |
| 前端巨型 surface | ✅ | share **100**、right-rail **41**、dashboard **337** 行 |
| CI 契约漂移门禁 | ✅ | `.github/workflows/frontend-unit.yml` 跑 `pnpm generate:contracts` + `git diff --exit-code` |
| 本地 drift 脚本 | ✅ | `pnpm check:contracts-drift` |

---

## 3. Findings

### 🔴 Critical

*（v1 的两项 Critical 已解决；v2 的 SSE/app 门面 Critical 残留亦已解决。当前无 Critical 级 finding。）*

---

### 🟡 Warning

#### Cognitive Overload — ReAct Agent Loop ~6009 行 / 17 模块

Symptom: `app-chat/src/agents/loop/` 共 **6009 行**（`mod.rs` **1089**、`iteration.rs` **1009**、`config.rs` **669**）；`ReActLoop` 持有多个 `Option<Arc<...>>` 依赖。

Source: Ousterhout — *A Philosophy of Software Design*, Deep vs Shallow Modules

Consequence: 修改 disclosure phase、exit policy 或 synthesis gate 需跨多个子模块理解；agent 迭代仍是最高认知负载区。

Remedy: 绘制 loop 状态机单页文档；将 `config`/`exit_policy`/`disclosure_plan` 合并为 `LoopPolicy` 深模块，对外只暴露 `run(request) -> AgentRunResult`。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**（部分来自 domain 固有复杂度）

---

#### Cognitive Overload — `mineru.rs` 1886 行 ingestion 解析器

Symptom: `ingestion/src/parser/mineru.rs` **1886 行**，为 workspace 最大单文件；PDF/OCR 路由逻辑集中。

Source: Fowler — *Refactoring*, Long Method; McConnell — *Code Complete*, Ch. 7

Consequence: ingestion 解析策略变更需在大文件中定位；与 `router.rs`（852 行）形成 parser 子系统热点。

Remedy: 按解析阶段（layout / table / figure / fallback）拆模块；`router.rs` 只做 dispatch。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**

---

#### Change Propagation — `common` 高 fan-in（24 crate / 163 文件）

Symptom: **24** 个 crate 依赖 `common`；**163** 个 `.rs` 文件含 `use common::`（**170** 行 import）；`common/src/lib.rs` 大量 `pub use contracts::...` 并追加 `rag_execute`、`tool_call` 等内部类型。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY; Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 内部类型变更波及全 workspace；新人不清楚新类型应放 `contracts` 还是 `common`。

Remedy: 明确分层：`contracts` = HTTP/API 边界；`common` = 仅 runtime 内部类型；移除 contracts re-export。

Priority: Pain 2 × Spread 3 = **6** | Intent: **[accidental]**

---

#### Dependency Disorder — Domain crate 直连 storage-pg（8+ crate）

Symptom: 除 `app-bootstrap` 外，`app`、`app-chat`、`app-admin`、`billing`、`share`、`chatmemory`、`admin` 均直接依赖 `avrag-storage-pg`。`app-chat` 新增 storage-pg 依赖（v2 未列出）。

Source: Martin — *Clean Architecture*, DIP

Consequence: 分层可测试性削弱；storage schema 变更可能波及 agent/chat 域。

Remedy: 经 repository trait/port 注入；仅 `app-bootstrap` 保留 concrete wiring。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[intentional]** composition root 模式，但范围过宽

---

#### Cognitive Overload — `handlers/notebooks.rs` 924 行

Symptom: handlers 拆分后 notebooks 仍为单文件 **924** 行；对比 `chat.rs` **474**、`documents.rs` **170**。

Source: Fowler — *Refactoring*, Long Method

Consequence: notebook CRUD / sessions / notes 改动在同一文件 merge conflict。

Remedy: 按 notebook CRUD / sessions / notes 再拆。

Priority: Pain 2 × Spread 2 = **4** | Intent: **[accidental]**

---

### 🟢 Suggestion

#### Change Propagation — `WorkspaceChatStreamEvent` kind 映射层仍存（已大幅改善）

Symptom: `stream.ts` **506** 行；`WorkspaceChatStreamEvent` 已通过 `WireToWorkspace<ChatEvent>` 从 generated type **派生**（`event`→`kind`），但 `parseWireChatEvent` / `chatEventToWorkspace` 仍 **~200 行**手写解析。`citations`/`done.payload` 有 runtime 窄化。

Source: Ousterhout — *A Philosophy of Software Design*, Information Leakage

Consequence: 协议字段变更时解析逻辑仍可能 drift；但 DTO 层已统一，风险较 v2 大幅降低。

Remedy: 评估 frontend reducer 直接消费 `ChatEvent`（`event` 字段），删除 `kind` 映射层；或 zod schema 从 fixture 生成。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[intentional]** UI 适配层

---

#### Cognitive Overload — `use-chat-stream.ts` 518 行

Symptom: chat 流式逻辑从 912 行 `use-chat-session` 拆出后集中在 `hooks/chat-session/use-chat-stream.ts` **518** 行。

Source: Fowler — *Refactoring*, Long Method

Consequence: SSE event reducer 仍集中；但已可独立测试。

Remedy: 按 event type 提取 reducer 函数（`handleTokenEvent`、`handleDoneEvent` 等）。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Accidental Complexity — `eval_framework.rs` 1633 行（feature-gate 但未剥离）

Symptom: `app-chat/src/agents/eval_framework.rs` **1633 行**；`eval` feature 默认关闭，但文件仍在主 crate 树中。

Remedy: 移入独立 crate 或 `tests/`。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[intentional?]**

---

#### Accidental Complexity — `rag_prompts.rs` 1739 行 prompt 字典

Symptom: 全部 RAG prompt 模板集中于单文件。

Remedy: 外置 YAML/模板文件；按 agent/mode 分目录。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Knowledge Duplication — `settings-share-messages.ts` 725 行平行 i18n

Symptom: 复制 `UiMessageDescriptor` 模式，与 `messages/settings.ts`、`messages/usage.ts` 键重叠。

Remedy: 合并进 `lib/i18n/messages/` 域分片。

Priority: Pain 2 × Spread 3 = **6** → 降为 Suggestion | Intent: **[accidental]**

---

#### Knowledge Duplication — Plus 用量倍数文案 6× vs 10× 不一致

Symptom: `messages/paywall.ts` 写 **10×**；`messages/usage.ts` 的 `usageUpgradeCta` 写 **6×**。

Remedy: 产品确认后统一；更新测试断言。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

#### Domain Model Distortion — `enterprise` DB 别名与 `UserTier` 过渡别名

Symptom: 应用逻辑已统一 `BillingTier::Free/Plus/Pro`；`react_loop.rs` 仍 `pub use BillingTier as UserTier`；admin-i18n 保留 legacy plan ID 映射。

Remedy: 逐步移除 `UserTier` 别名。

Priority: Pain 1 × Spread 1 = **1** | Intent: **[intentional]**

---

#### Accidental Complexity — contracts 非 chat 模块缺 JSON 往返测试

Symptom: 仅 `contracts/tests/chat_json.rs`；notebooks/billing/admin 等无同级 golden fixture。

Remedy: 扩展 `export_golden_fixtures` 至其他模块。

Priority: Pain 1 × Spread 2 = **2** | Intent: **[accidental]**

---

## 4. Debt Summary

| Risk | Findings | Avg Priority | Classification | Intent |
|------|----------|-------------|----------------|--------|
| Cognitive Overload | 4 | 3.5 | Scheduled | accidental |
| Change Propagation | 2 | 4.0 | Monitored | mixed |
| Knowledge Duplication | 2 | 2.0 | Monitored | accidental |
| Accidental Complexity | 3 | 2.0 | Monitored | mixed |
| Dependency Disorder | 1 | 4.0 | Scheduled | intentional |
| Domain Model Distortion | 1 | 1.0 | **Resolved** | intentional |

**Recommended focus:** Cognitive Overload（ReAct loop + mineru parser）→ Change Propagation（`common` fan-in 分层）

---

## 5. 偿还路线图（v3 更新）

### 已完成 ✅（v1–v2 路线图）

| 任务 | 验收 |
|------|------|
| worker `main.rs` 拆分 | 4 行 |
| `pipeline/helpers.rs` 拆分 | 255 行 |
| contracts → TS codegen + CI drift gate | `frontend-unit.yml` |
| billing tier 统一 | `BillingTier` |
| handlers 按域拆分 | 5 文件 |
| transport 改 `app-bootstrap` | 0 处 `app::AppState` |
| SSE 接入 `ChatEvent` | `WireToWorkspace` 派生 |
| 前端巨型 surface / chat hook 拆分 | share 100、use-chat-session 95 |
| `UiChatMessage` 重命名 | `chat-session/types.ts` |

### Phase 3 — 当前优先

| 任务 | 验收 |
|------|------|
| ReAct loop 状态机文档 + `LoopPolicy` 深模块 | 对外接口 ≤3 方法 |
| `mineru.rs` 按解析阶段拆分 | 单文件 <500 行 |
| `common` 分层：移除 contracts re-export | import 路径清晰 |
| `notebooks.rs` 再拆 | 单文件 <500 行 |
| 合并 `settings-share-messages.ts` | 进 i18n 域分片 |
| 统一 Plus 倍数文案 | 产品确认后 |

### Phase 4 — 后续

- 评估删除 `WorkspaceChatStreamEvent` kind 层
- `eval_framework` 移入独立 crate
- `rag_prompts.rs` 外置模板
- 扩展 contracts golden fixtures

### 集成门禁

```bash
cd avrag-rs && cargo test -p contracts -p app-chat -p transport-http -p avrag-billing
cd frontend_next && pnpm check:contracts-drift && pnpm typecheck && pnpm test tests/contracts/
./scripts/check_contract_governance.sh
```

---

## 6. 预期成果

| 维度 | v2 | Phase 3 完成后预期 |
|------|-----|-------------------|
| Health Score | 58 | **78–82** |
| Critical | 0 | 0 |
| Warning | 8 | ≤4 |
| 最大单文件 | mineru 1886 | <800 行（除测试） |

---

## 7. 附录：关键文件索引

| 路径 | 行数 | 说明 |
|------|------|------|
| `ingestion/src/parser/mineru.rs` | 1886 | 最大生产文件 |
| `app-chat/src/agents/loop/` | 6009 | ReAct loop 子系统 |
| `app-chat/src/rag_prompts.rs` | 1739 | Prompt 字典 |
| `app-chat/src/agents/eval_framework.rs` | 1633 | eval（feature-gate） |
| `transport-http/src/handlers/notebooks.rs` | 924 | handler 热点 |
| `frontend_next/lib/workspace/stream.ts` | 506 | SSE 解析（已接 ChatEvent） |
| `frontend_next/hooks/chat-session/use-chat-stream.ts` | 518 | chat 流式 hook |
| `frontend_next/lib/settings-share-messages.ts` | 725 | 平行 i18n |
| `bins/worker/src/pipeline/helpers.rs` | 255 | ✅ 已拆分 |
| `.github/workflows/frontend-unit.yml` | — | CI codegen drift |
| `.brooks-lint-history.json` | — | Brooks-Lint 历史 |

---

*生成工具：Brooks-Lint Tech Debt Assessment · 2026-06-12 v3（深入探测）*
