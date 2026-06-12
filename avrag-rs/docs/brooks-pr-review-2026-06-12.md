# Brooks PR 代码审查 — 2026-06-12

Brooks-Lint **PR Review** 对当前工作区未提交变更的深度审计结论。范围：196 个已跟踪文件（+2315 / −34653 行），对 `app` 拆分、worker、transport-http、前端 workspace、测试做高风险区抽样审查；跳过 `frontend_next/out/` 等构建产物。

**Mode:** PR Review  
**Scope:** 工作区未提交变更 vs HEAD（抽样审查高风险区域）  
**Health Score:** 43/100  
**Trend:** 首次 PR Review 运行（无历史对比）

**一句话结论：** `app` 向多 crate 分解的架构方向正确，但当前工作区无法通过 `cargo check --workspace`，且 6 个新 domain crate 仍为未跟踪文件——**尚不可合并**。

---

## 1. 变更概览

| 维度 | 数据 |
|------|------|
| 已跟踪文件 | 196 |
| 行数变化 | +2,315 / −34,653 |
| 主要方向 | `app` → `app-core` / `app-chat` / `app-documents` / `app-admin` / `app-billing` / `app-bootstrap`；worker PDF/索引模块化；前端 workspace 数据逻辑抽到 hook |
| 未跟踪关键路径 | `app-chat/`、`app-core/`、`app-bootstrap/`、`app-documents/`、`app-admin/`、`app-billing/`、`chat_delegates.rs`、`delegate_contract.rs`、`use-workspace-data.ts` 等（约 42 项） |

### Crate 职责（参考 [t13-app-split-inventory.md](./t13-app-split-inventory.md)）

| Crate | 职责 |
|-------|------|
| `app-core` | 共享上下文、config、domain ports |
| `app-chat` | Chat pipeline、agents、sessions、citations、RAG execute |
| `app-documents` | Notebooks、documents、ingest、URL imports |
| `app-admin` | API keys、notifications、admin 操作 |
| `app-billing` | Usage limits、quota checks |
| `app-bootstrap` | `new_memory` / `bootstrap`、工厂 wiring |
| `app` | 薄 facade：`AppState` delegates + HTTP 面向 re-exports |

---

## 2. Findings

### 2.1 Critical

#### Dependency Disorder — `storage-pg` / `app-bootstrap` 依赖断裂导致全 workspace 编译失败

| 字段 | 内容 |
|------|------|
| **Symptom** | 审计时 `avrag-storage-pg` 将 `ingestion` 从 `[dependencies]` 移至 `[dev-dependencies]`，但 `core.rs` 仍 `use ingestion::{...}`；后续局部修复为 `ingestion_types::`，但 `cargo check --workspace` 仍因 `app-bootstrap` 失败：`Cargo.toml` 缺少 `async-trait`、`chrono`、`common` 等；adapter 使用私有路径 `app_core::config_helpers::map_pg_error`（`config_helpers` 在 `app-core` 中为 `mod`，仅根 re-export 公开）。 |
| **Source** | Martin — *Clean Architecture*, Dependency Inversion Principle; Winters et al. — *Software Engineering at Google*, Ch. 21 Dependency Management |
| **Consequence** | 任何基于当前 diff 的 CI/本地构建失败；`app-bootstrap` → `storage-pg` 链路阻断，拆分工作无法验证。 |
| **Remedy** | `app-bootstrap/Cargo.toml` 补齐依赖；adapter import 改为 `app_core::map_pg_error`；验证 `cargo check --workspace` 通过。 |

#### Change Propagation — PR 不完整：核心新 crate 未纳入版本控制

| 字段 | 内容 |
|------|------|
| **Symptom** | `git status` 显示 6 个关键 crate 仍为 `??` 未跟踪：`app-chat/`、`app-core/`、`app-bootstrap/`、`app-documents/`、`app-admin/`、`app-billing/`。已跟踪 diff 大量删除 `crates/app/src/agents/*`、`chat/*` 等，替代实现不在提交范围内。 |
| **Source** | Brooks — *The Mythical Man-Month*, Ch. 2 Brooks's Law; Fowler — *Refactoring*, Shotgun Surgery |
| **Consequence** | 协作者或 CI checkout 该分支后得到残缺代码库；审查看到的「删除」无法对应「新增」，合并风险不可评估。 |
| **Remedy** | 合并前 `git add` 所有新 crate 与依赖文件；按 inventory 文档拆成可独立审查的子 PR（若团队流程允许）。 |

---

### 2.2 Warning

#### Change Propagation — 超大规模 PR 本身即架构异味

| 字段 | 内容 |
|------|------|
| **Symptom** | 196 个文件、净删 3.2 万行，横跨 Rust 后端拆分、worker PDF 管线、ingestion 路由、contracts、前端 billing/workspace、E2E 测试与文档。 |
| **Source** | Brooks — *The Mythical Man-Month*, Conceptual Integrity; Ousterhout — *A Philosophy of Software Design*, Information Leakage |
| **Consequence** | 单次审查无法覆盖所有行为变更；回归缺陷极易藏在相邻文件的「顺手修改」中；回滚粒度粗。 |
| **Remedy** | 按 crate 边界拆分 PR（`app-core`+`app-bootstrap` → `app-chat` → facade delegates → worker/pdf → 前端 hook），每步附带测试门禁。 |

#### Knowledge Duplication — `AppState` facade 与 `ChatContext` 双份维护

| 字段 | 内容 |
|------|------|
| **Symptom** | [`chat_delegates.rs`](../crates/app/src/lib_impl/chat_delegates.rs)（170 行）对每个 chat/RAG/agent 方法做 `self.chat_ctx().method()` 透传；`chat_ctx()` 在 21 处被调用，每次重建含 8 个字段的 `ChatContext`。 |
| **Source** | Fowler — *Refactoring*, Middle Man; Hunt & Thomas — *The Pragmatic Programmer*, DRY |
| **Consequence** | 每新增 chat 能力需同时改 `app-chat` 与 `app` facade；漏改一侧即产生 Hyrum's Law 下的静默行为分叉。 |
| **Remedy** | 在 bootstrap 时预构建 `ChatContext` 并存入 `AppState`；扩展 [`delegate_contract.rs`](../crates/app/tests/delegate_contract.rs) 覆盖 chat/RAG 主路径。 |

#### Cognitive Overload — `worker/main.rs` 拆分后仍超 3200 行

| 字段 | 内容 |
|------|------|
| **Symptom** | PDF/索引逻辑已提取到 `bins/worker/src/pdf/` 与 `indexing/`，但 `main.rs` 仍有 3263 行，继续承载 ingestion 管线、图索引、多模态、任务处理等。 |
| **Source** | McConnell — *Code Complete*, Ch. 7; Ousterhout — *A Philosophy of Software Design*, Deep Modules |
| **Consequence** | PDF 拆分收益被文件体量抵消；后续改 ingestion 路由或索引策略仍需在巨型文件中导航。 |
| **Remedy** | 将 `PgTaskProcessor`、`run_document_pipeline`、`execute_parse_plan` 等移入 `worker/src/pipeline/` 子模块，让 `main.rs` 仅保留启动与 wiring。 |

#### Coverage Illusion — chat facade 缺少契约测试

| 字段 | 内容 |
|------|------|
| **Symptom** | 新增 `delegate_contract.rs` 覆盖 citation 与 admin，但 `execute_chat`、`execute_chat_stream`、`list_sessions` 等 20+ 个 chat delegate 无对应契约测试；agents 模块测试随 `app` 删除，迁移到 `app-chat` 的覆盖未在本次 diff 中完整体现。 |
| **Source** | Feathers — *Working Effectively with Legacy Code*, Ch. 1; Google — *How Google Tests Software*, change coverage |
| **Consequence** | facade 透传层回归无法被快速捕获；拆分后最容易坏的是「看起来只是转发」的边界。 |
| **Remedy** | 为 `execute_chat`（memory 模式）、`list_sessions`、`execute_rag_execute_plan` 各补契约测试，断言错误码与 HTTP handler 入口一致。 |

#### Cognitive Overload — `transport-http/handlers.rs` 持续膨胀

| 字段 | 内容 |
|------|------|
| **Symptom** | 文件 1834 行；notebook analysis 逻辑提取为文件内 `NotebookAnalysisCollector`（~120 行 impl），但仍与路由处理混处同一文件。 |
| **Source** | Fowler — *Refactoring*, Long Method; Martin — *Clean Architecture*, SRP |
| **Consequence** | HTTP 层承载越来越多领域聚合逻辑，与「app 拆分」目标相悖。 |
| **Remedy** | 将 `NotebookAnalysisCollector` 移到 `transport-http/src/notebook_analysis.rs` 或 `app-documents` 领域服务。 |

---

### 2.3 Suggestion

#### Dependency Disorder — 过时的 `billing_context` 模块引用

| 字段 | 内容 |
|------|------|
| **Symptom** | [`state_methods.rs:80`](../crates/app/src/lib_impl/state_methods.rs) 返回类型仍为 `&crate::billing_context::BillingContext`，但 `billing_context` 模块已从 `app` 删除，实际字段类型为 `app_billing::BillingContext`。 |
| **Source** | Evans — *Domain-Driven Design*, Bounded Context |
| **Consequence** | 修复 `storage-pg` / `app-bootstrap` 后此处将成为下一个编译错误。 |
| **Remedy** | 改为 `&app_billing::BillingContext`。 |

#### Accidental Complexity — crate 级 `#![allow(dead_code)]` 掩盖腐烂

| 字段 | 内容 |
|------|------|
| **Symptom** | [`app/src/lib.rs`](../crates/app/src/lib.rs) 顶部保留 `#![allow(dead_code)]`、`#![allow(deprecated)]`、`#![allow(unused_mut)]`。 |
| **Source** | McConnell — *Code Complete*; Ousterhout — *A Philosophy of Software Design*, tactical programming |
| **Consequence** | 拆分后遗留的未使用 facade/delegate 代码不会触发编译器警告，债务静默累积。 |
| **Remedy** | 拆分完成后移除全局 allow，改为逐处 `#[allow]` 或删除死代码。 |

---

## 3. 正向观察

- **架构方向正确：** 将臃肿 `app` 拆成领域 crate，用 facade delegate 保持 `transport-http` 入口稳定，符合 [t13-app-split-inventory.md](./t13-app-split-inventory.md) 设计。
- **前端减负有效：** workspace 数据逻辑抽到 [`use-workspace-data.ts`](../../frontend_next/hooks/use-workspace-data.ts)，`workspace-surface.test.tsx` 12 项测试仍通过。
- **契约测试起步：** `delegate_contract.rs` 为 citation/admin 建立了可复制的测试模式。

---

## 4. 推荐修复顺序

1. 纳入未跟踪 crate + 修复 workspace 编译（`app-bootstrap` 依赖、`map_pg_error` 路径、`billing_context` 类型）
2. 跑通 workspace 测试（见 inventory Verify 段）
3. 补 chat delegate 契约测试
4. `ChatContext` bootstrap 预构建 + 移除 crate 级 allow
5. 抽出 `NotebookAnalysisCollector`、拆分 worker `pipeline/*`
6. （流程层面）按 crate 边界拆小 PR 以利 review

### 验证命令（来自 inventory）

```bash
cd avrag-rs
cargo check --workspace
cargo test -p app-core -p app-billing -p app-documents -p app-chat -p app-admin -p app-bootstrap -p app --lib
cargo test -p transport-http
```

```bash
cd frontend_next
pnpm exec vitest run tests/workspace/workspace-surface.test.tsx
```

---

## 5. Summary

本次变更的核心价值在于 AppState 分解与 ingestion/worker 模块化，但当前状态**不能合并**：构建断裂、新 crate 未入库、196 文件巨型 PR 使审查无法建立信心。优先修复构建并完整提交新 crate，再按 inventory 推进模块化与测试补强。

完整测试套件诊断可参考同日文档：[brooks-test-quality-review-2026-06-12.md](./brooks-test-quality-review-2026-06-12.md)。

---

## 6. 历史记录

| 日期 | Mode | Score | Scope |
|------|------|-------|-------|
| 2026-06-12 | PR Review | 43 | 工作区 196 文件（抽样） |
| 2026-06-12 | Tech Debt Assessment | 34 | avrag-rs + frontend_next + contracts |
| 2026-06-11 | Health Dashboard | 80 | ingestion routing v2 |

记录来源：项目根 [`.brooks-lint-history.json`](../../.brooks-lint-history.json)
