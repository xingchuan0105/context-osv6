# 修复执行计划 — Thermo-Nuclear Review 2026-07-08

> 基于 [THERMO_NUCLEAR_REVIEW_2026-07-08.md](/home/chuan/context-osv6/THERMO_NUCLEAR_REVIEW_2026-07-08.md) 的 6 CRITICAL + 25 HIGH 发现。
> 按里程碑（Milestone）编排，每个里程碑有独立的验证门和合并点。可在任意里程碑后停止。

---

## 用户已确认的 4 项决策

1. ✅ **`AgentRunResult.snapshot` 字段** → **彻底删除**（含 `ReplaySnapshot` 类型 + serde 字段，不做保守保留）
2. ✅ **`AVRAG_AUTH_VERSION_BYPASS`** → **已确认：无测试脚本调用，无 .env 配置**。该机制仅在 memory mode（无 DB）下触发。处理方案：删除环境变量检查，memory mode 下 `jwt_auth_version_matches` 直接返回 `true`（memory mode 本身就是开发专用无安全语义的模式）。归入 M4c。
3. ✅ **测试容器** → **复用**（删除 `e2e-precheck.sh` 中的 `docker rm -f`，遵从 AGENTS.md 不变量）
4. ✅ **M4d（提取 app-storage-pg crate）** → **现在做**，与 W3b 合并为统一大任务（先拆结构体再搬 crate）

---

## 依赖图与并行总览

```
M0 零风险删除 (~2000行)  ─┐
M1 契约完整性 (CRITICAL)  ├── 全部独立，可并行启动
M2 前端卫生               ─┘
M3 模块墙 (include!→mod)  ── 关键路径，解锁 M4/M5
    └─ W3b 与 M4d 合并：先拆 PgAppRepository→8 结构体，再提取 app-storage-pg crate
M4 逻辑提取              ── 依赖 M3
M5 复制粘贴坍缩           ── 部分依赖 M3，长尾
```

**并行策略：** M0 + M1 + M2 + M5 的独立部分可同时用 subagent 并行执行。M3 是串行关键路径。

**分支策略：** 每个里程碑一个分支（如 `fix/tn-m0-deadcode`），里程碑内每个 workstream 可独立 commit。从 M0 开始串行合并，避免大面积冲突。

---

## 通用验证门（每个 workstream 完成后执行）

```bash
# Rust 后端（在 avrag-rs/）
cargo test --workspace
cargo clippy --all-targets -- -D warnings

# 契约（在 contracts/）
cargo test

# 前端（在 frontend_next/）
pnpm typecheck && pnpm test

# 契约 codegen 漂移（在 frontend_next/）
pnpm generate:contracts && git -C .. diff --exit-code -- frontend_next/lib/contracts/generated/
```

---

## M0 — 零风险删除（~2,000 行，零行为变化）

**目标：** 删除死代码/投机架构/孤立文件，降低后续重构的认知噪音。
**风险：** 极低（纯删除，编译器保证行为不变）。
**可并行：** W0a / W0b / W0c / W0d 全部独立。

### W0a — app-chat 死代码（~1,050 行）+ 彻底删除 snapshot
- **彻底删除** `crates/app-chat/src/agents/replay.rs` 全文（725 行），包括 `ReplaySnapshot` 类型定义。
- **从 `AgentRunResult`（`runtime.rs:271-295`）中删除 `snapshot` 字段** + 其 serde 属性。
- **从 `run_result.rs:97-102` 中删除 `snapshot: None` 赋值。**
- **删除** `crates/app-chat/src/agents/audit.rs` 中 6 个未调用 builder（`high_risk_tool_call_record`, `policy_deny_record`, `policy_approval_record`, `budget_exhausted_record`, `degrade_event_record`, `permission_denied_record`）+ `AuditLifecycleManager`, `AuditSinkAdapter`, `InMemoryAuditStorage`, 两个 `AuditStorage` trait。仅保留 `routing_decision_record`（唯一生产调用点：`unified/mod.rs:114`）。
- **删除** `CapabilityRegistry::answer_behavior_modes`（零调用）+ `answer_format_skills`（仅测试断言"不存在"）。
- **同时清理 `AgentRunResult` 中其他死字段**（`trace`, `decisions`, `tool_calls`, `eval_summary` — 生产中从未赋值）及其支撑类型（`AgentTrace`, `TraceSpan`），减小 struct 体积。
- **验证：** `cargo test -p avrag_app_chat` + `cargo clippy -p avrag_app_chat -- -D warnings`（确认无 unused warning 残留引用）。需确认 serde fixture 测试（`tests/chat_json.rs`）中不含 `snapshot`/`trace` 等字段，若有则更新。

### W0b — app crate 残余架构（~700 行）
- **删除** `crates/app/src/services/secure_services.rs`, `secure_service_impls.rs`, `services/security/xml_slot_engine.rs`, `crates/app/src/runtime/`（`bootstrap.rs`, `container.rs`）。
- 保留 `crates/app/src/lib.rs` 中对 `app_bootstrap` 的 re-export（`AppConfig`, `AppState`）——这是 `bins/api` 唯一使用的入口。
- **验证：** `cargo build -p avrag_app` + `cargo build -p avrag_api`（确认 bin 仍编译）。

### W0c — 死脚本 + 已跟踪死文件（~105 文件）+ 测试容器复用
- **删除** 5 个死 rebase 脚本：`scripts/_commit-prs.sh`, `_continue-reword.sh`, `_reword-task23.sh`, `_recommit-pr5-pr6.sh`, `pricing-revamp-commits.sh`。
- **删除** 孤立 Python patcher：`scripts/patch-chat-contract-codegen.py`, `scripts/annotate-contract-typeshare-integers.py`（确认注解已冻结到源码后删——由 M1 验证）。
- **删除** `scripts/e2e-precheck.sh:34-36` 中的 `docker rm -f avrag-test-pg-*` 行（遵从用户决定：测试容器复用）。
- **git rm** `avrag-rs/prompts/{_backups,_drafts,deprecated,legacy}/`（98 文件）+ `avrag-rs/python/avrag_sdk/{egg-info,benchmark_output}`（7 文件）。
- **更新** `.gitignore`：添加 `**/egg-info/`, `benchmark_output/`, `prompts/_backups/`, `prompts/_drafts/`。
- **验证：** `git status` 确认无残留引用；`scripts/generate-contracts.sh` 仍可运行（patcher 已不在其中）。

### W0d — 死 PG 全文搜索（~100 行）
- **删除** `crates/storage-pg/src/lib_impl/repository_retrieval.rs` 中 `search_chunks_text`, `search_chunks_bm25`（零调用，Milvus 已接管全部检索）。
- **验证：** `cargo test -p avrag_storage_pg`。

### M0 合并门
- 全部 4 个 workstream 通过各自验证后，跑一次完整 `cargo test --workspace` + `cargo clippy --all-targets -- -D warnings`。
- 合并 `fix/tn-m0-deadcode`。

---

## M1 — 契约完整性（修复 CRITICAL 生产风险）

**目标：** 消除静默 codegen 漂移，让 TS 契约类型真正可靠。
**风险：** 中（改 codegen 管线，但每个改动可独立验证）。
**可并行：** W1a → W1b → W1c 有顺序依赖。

### W1a — 修复 agent_operation_guide 漂移（CRITICAL）
- 在 `contracts/src/tool_call.rs:10` 的 `ToolSpec` 上添加 `#[typeshare]`。
- 在 `contracts/src/chat.rs` 的 `AnswerBlock`（169-184 行，当前无 `#[typeshare]`）上添加 `#[typeshare]`。
- **删除** `scripts/generate-contracts.sh` 中的 inline Python heredoc（注入 `import type { AnswerBlock }`，20-38 行）和 3 行 sed 重写（48-53 行）——因为 typeshare 现在能原生定义这两个类型。
- **验证：**
  - `cd contracts && cargo test`
  - `cd frontend_next && pnpm generate:contracts`
  - `grep AgentOperationGuide frontend_next/lib/contracts/generated/contracts.ts` → 应有匹配
  - `grep AnswerBlock frontend_next/lib/contracts/generated/contracts.ts` → 应有定义

### W1b — 添加 TS key-completeness 金标准测试
- 在 `frontend_next/tests/` 新增 `contract-completeness.test.ts`：
  ```ts
  import type { ChatResponse } from '../lib/contracts/generated/contracts';
  // 断言 ChatResponse 的 key 集合与 Rust 源完全一致
  ```
  对比 Rust `ChatResponse` 字段列表（从 `chat.rs:670` 提取）与 `Object.keys(fixture)`。
- **验证：** `pnpm test contract-completeness`。

### W1c — 消除 92 个冗余整数注解
- 先做实验：删除一个 `#[typeshare(serialized_as = "number")]`，运行 `pnpm generate:contracts`，`git diff` 确认 `contracts.ts` 无变化（证明 `typeshare.toml` 的 `[typescript.type_mappings]` 已覆盖）。
- 若实验通过：批量删除所有 92 个 `serialized_as = "number"` 注解。
- 若实验失败（toml 映射不生效）：则反过来——删除 `typeshare.toml` 中的映射块，保留注解。二选一，不能并存。
- **验证：** `pnpm generate:contracts && git diff --exit-code -- ../frontend_next/lib/contracts/generated/`。

### M1 合并门
- `pnpm check:contracts-drift` 通过。
- `cd contracts && cargo test` 通过。
- 合并 `fix/tn-m1-contracts`。

---

## M2 — 前端卫生（独立于后端）

**目标：** 消除前端复制粘贴，让生成契约类型在边界真正可靠。
**风险：** 低-中（纯前端，`pnpm test` + `pnpm typecheck` 保护）。
**可并行：** W2a / W2b 独立；W2c 依赖 W1a 完成（需要完整契约类型）；W2d 独立。

### W2a — 统一 ApiEnvelope + unwrapApiData
- 在 `frontend_next/lib/http/request.ts` 新增：
  ```ts
  export async function requestEnvelope<T>(path, init, token): Promise<T> {
    const env = await request<ApiEnvelope<T>>(path, init, token);
    return unwrapApiData(env);
  }
  ```
- 删除 `lib/admin/client.ts:118-124,159-165`, `lib/share/client.ts:69-81`, `lib/settings/client.ts:106-112,150-156` 中的重复 `ApiEnvelope` 定义和 `unwrapApiData`。
- 所有调用点 `unwrapApiData(await request<ApiEnvelope<X>>(...))` → `requestEnvelope<X>(...)`。
- **验证：** `pnpm typecheck && pnpm test`。

### W2b — 统一 notebook→workspace 映射器
- 在 `frontend_next/lib/workspace/client.ts` 提取共享 `mapWorkspace(raw): Workspace`。
- `lib/dashboard/client.ts:35-73` 的 `RawWorkspace`/`mapWorkspace` 改为引用共享版本。
- 删除 `workspace/client.ts` 中的 `remapWorkspace` 泛型 + inline `{id, ...notebook}` 展开。
- **验证：** `pnpm typecheck && pnpm test`。

### W2c — SSE 解析器 zod schema（依赖 W1a）
- 在 `frontend_next/lib/workspace/stream.ts` 中，为每个 `ChatEvent` 变体定义 zod schema（从 `contracts.ts` 推导）。
- 替换 `parseWireChatEvent`（125-245 行）的手动 `String(raw.doc_id ?? "")` 强制转换 + `as unknown as Array<Record<string, unknown>>` 双重转型。
- 使用 `schema.safeParse(raw)`，失败时显式拒绝。
- **验证：** `pnpm test stream`。

### W2d — 分解上帝组件
- **`workspace-note-editor-tiptap.tsx`（852 行）：**
  - 删除手写 HTML 消毒器（48-153 行），改用 `DOMPurify.sanitize(html, WORKSPACE_HTML_SANITIZE_CONFIG)`（`citation-renderer.tsx:62` 已有先例）。
  - 提取 `workspace-note-link-panel.tsx`（面板 + 定位几何）。
  - 提取 `workspace-editor-toolbar.tsx`（toolbar 配置/图标）。
  - 修复 CSS 导入：从 `workspace-right-rail.module.css` 改为独立的 `workspace-note-editor.module.css`。
- **`workspace-history-pane.tsx`（743 行）：**
  - 提取 `lib/workspace/session-title-text.ts`（纯字符串工具，~120 行）。
  - 提取 `hooks/use-session-transcripts.ts`：一次拉取会话消息，同时派生标题和搜索文档，消除 3× 重复拉取。
  - 统一 CSS：搜索模态框改用 CSS module（或统一用全局类）。
- **验证：** `pnpm typecheck && pnpm test`。

### M2 合并门
- `pnpm typecheck && pnpm test` 全绿。
- 合并 `fix/tn-m2-frontend`。

---

## M3 — 模块墙 + 存储层重构（include! → mod + 拆 crate，关键路径）

**目标：** 消除 `include!` 扁平命名空间，恢复模块可见性边界，并将 PgAppRepository 拆分后提取为独立 crate。这是解锁 M4/M5 的前提。
**风险：** 高（大面积重构，但行为不变，测试保护）。
**顺序：** W3a 先（transport-http 较小）；W3b/W3c 合并为一个大任务（storage-pg：先拆结构体再搬 crate）。

### W3a — transport-http/lib_impl.rs → mod 树
- 将 `lib_impl.rs` 中的 7 个 `include!` 转为真实模块：
  - `lib_impl/router_core.rs` → `mod router_core;`
  - `lib_impl/auth_primary.rs` → `mod auth_primary;`（或 `mod auth { mod primary; }`）
  - `lib_impl/auth/{profile,preferences,reset}.rs` → `mod auth { mod profile; mod preferences; mod reset; }`
  - `lib_impl/infra_handlers.rs` → `mod infra_handlers;`
  - `lib_impl/tests.rs` → 保留为 `#[cfg(test)] mod tests;`
- 将扁平调用改为显式 `use` 路径。编译器会报错指引所有需要改的调用点。
- 添加 `pub(crate)` 可见性标注，让 auth 层不能随意访问 router 内部。
- **验证：** `cargo test -p avrag_transport_http` + `cargo clippy -p avrag_transport_http -- -D warnings`。

### W3b+W3c — storage-pg: 拆 PgAppRepository → 8 结构体 + 提取 app-storage-pg crate（合并大任务）

**用户决定：现在做，与 W3b 合并。** 分三个 commit 推进：

**Commit 1 — include! → 真实 mod（纯模块化，不拆结构体）：**
- 将 `storage-pg/src/lib_impl.rs` 的 24 个 `include!` 转为 `mod` 声明。
- 修正 `pub(crate)` 可见性。
- 编译器指引所有路径修正。
- **验证：** `cargo test -p avrag_storage_pg` 编译通过。

**Commit 2 — 拆 PgAppRepository → ~8 个聚焦结构体：**
- 按领域聚合拆分：
  - `DocumentRepository`（文档 CRUD + IR + body + status 变更，含原 `repository_retrieval_lifecycle.rs`）
  - `ChunkRepository`（chunk 存储 + ContentStore 检索方法）
  - `SessionRepository`（会话 + 消息 + ChatPersistencePort 方法）
  - `IngestionQueueRepository`（摄入队列 + 清理队列）
  - `AssetRepository`（资产）
  - `AuthRepository`（用户 + 认证 + legal acceptance）
  - `BillingRepository`（计费 + 配额 + 用量限制）
  - `BootstrapRepository`（connect/migrate/ping + notebook CRUD）
- 每个持有 `Arc<TenantPgPool>`。
- 更新 `app-bootstrap/src/adapters/` 中的引用（适配器改为持有对应的 Repository）。
- 重命名/删除误导性命名的文件。
- **验证：** `cargo test --workspace` + `cargo clippy --workspace -- -D warnings`。

**Commit 3 — 提取 app-storage-pg 独立 crate（原 M4d）：**
- 将 Commit 2 拆好的 8 个 Repository 结构体 + 相关 SQL 从 `app-bootstrap/src/adapters/` 和 `storage-pg` 移入新 crate `app-storage-pg`。
- `app-bootstrap/src/lib.rs` 只保留 wiring（依赖注入组装）。
- 更新 workspace `Cargo.toml`。
- **验证：** `cargo test --workspace` + `cargo clippy --all-targets -- -D warnings`。

### M3 合并门
- `cargo test --workspace` + `cargo clippy --all-targets -- -D warnings` 全绿。
- 合并 `fix/tn-m3-mod-walls`。

---

## M4 — 逻辑提取（依赖 M3）

**目标：** 业务逻辑从 handler/wiring 层移到 service 层，实现 Memory*Store port。
**风险：** 中-高（跨层重构）。
**可并行：** W4c（Memory store）独立于 M3；W4a/W4b 依赖 W3a。

### W4a — PasswordResetService（依赖 W3a）
- 新建 `crates/app/src/services/password_reset.rs`（或放入 `app-bootstrap`）：
  ```rust
  pub struct PasswordResetService { config: PasswordResetConfig, store: ..., mailer: ... }
  impl PasswordResetService {
      pub async fn request_reset(&self, email) -> Outcome { ... }
      pub async fn verify_and_reset(&self, code, new_pw) -> Outcome { ... }
  }
  ```
- `config.from_env()` 在 bootstrap 时读一次，注入 service。
- `transport-http` 的 `reset.rs` handler 变为 ~10 行 thin controller。
- **验证：** `cargo test -p avrag_transport_http`（auth reset 测试）。

### W4b — BillingService（依赖 W3a）
- 将 `billing/src/api.rs` 重命名为 `billing/src/service.rs`。
- `BillingConfig::from_env()` + provider clients 在构造时注入，不再每请求重读。
- 保留薄 `handlers.rs`（真正的 axum wiring）。
- **验证：** `cargo test -p avrag_billing`。

### W4c — Memory*Store port 实现 + AUTH_VERSION_BYPASS 清理（S4，独立于 M3）
- 在 `crates/app-core/src/adapters/memory.rs` 新增：
  - `MemoryDocumentStore: DocumentStorePort`（持有 `Arc<RwLock<MemoryState>>`）
  - `MemoryAdminStore: AdminStorePort`
- 修改 `app-bootstrap` 的 `new_memory()` 构造真实的 port impl（不再依赖 `Option<store>` + inline memory fallback）。
- 删除 `app-documents/src/documents.rs` 等 ~30 个方法中的 `if let Some(store) { ... } else { memory }` 双分支。
- **AUTH_VERSION_BYPASS 清理（用户决策 #2）：** 在 `postgres_delegates.rs:311-339`，删除 `AVRAG_AUTH_VERSION_BYPASS` 环境变量检查。memory mode（`postgres_repo()` 返回 None）下 `jwt_auth_version_matches` 直接返回 `true`（memory mode 本身是开发专用无安全语义的模式）。删除 `AVRAG_AUTH_VERSION_BYPASS` 变量名及注释（310 行）。
- **验证：** `cargo test -p avrag_app_documents` + `cargo test -p avrag_app_admin`（memory mode 测试必须全绿）。

### M4 合并门
- `cargo test --workspace` + `cargo clippy --all-targets -- -D warnings` 全绿。
- E2E 冒烟测试（`bash scripts/run-e2e.sh` 或至少 password-reset + billing flow）。
- 合并 `fix/tn-m4-logic-extraction`。

---

## M5 — 复制粘贴坍缩（长尾去重）

**目标：** 消除剩余的复制粘贴和模式重复。
**风险：** 低-中（局部重构，各自独立）。
**可并行：** 大部分独立。W5a 依赖 W3a。

### W5a — 统一错误/响应 + auth extractor（依赖 W3a）
- 给 `AppError` 一个 `IntoResponse` impl（放 `common`），删除 `handlers/mod.rs` 和 `routes/admin.rs` 中的两份 `app_error_response`。
- 选定一种成功信封（或用裸 `(StatusCode, Json(T))`），删除 `AuthEnvelope` / `ApiEnvelope` 分裂。
- 新建 axum extractor `RequireSession(State) -> (&AppState, &dyn AuthStore, UserId)`，消除 12× auth_store guard + 7× 三段 guard 复制。
- **验证：** `cargo test -p avrag_transport_http`。

### W5b — Stage trait + IngestionError 分类（独立）
- 在 `bins/worker/src/pipeline/` 定义 `trait Stage { async fn run(&mut self, ctx: &mut PipelineContext) -> Result<()> }`。
- `run_document_pipeline_inner`（620 行）→ `for stage in stages { stage.run(&mut ctx).await? }`。
- 将 `IngestionError::StateSink(String)` 拆为分类变体（`Storage`, `Parse`, `Security`, `Index`），添加 `From` impl，删除 32× `.map_err(|e| StateSink(e.to_string()))`。
- **验证：** `cargo test -p avrag_worker`。

### W5c — ReActLoop RunFinalizer（独立，app-chat）
- 引入 `struct RunFinalizer { iteration, max_iterations, total_tool_calls, telemetry_records, total_usage, reasoning_summary_acc, start_time, ... }`。
- 6 个终端阶段方法（`resolve_synthesis_gate`, `finish_direct_answer_run`, `run_synthesis_phase`, 等）从 10-17 参数降至 2-3 参数。
- 提取 `unified/mod.rs` 三臂 dispatch 为 `run_react_mode(mode_id, llm, configure_closure, ...)`。
- **验证：** `cargo test -p avrag_app_chat`。

### W5d — run_channel 泛型 + 去重（独立，rag-core）
- `channels.rs`：一个泛型 `async fn run_channel<Fut, T>(stage, fut) -> (T, ChannelTraceItem, Vec<DegradeTraceItem>)`，返回共享 `ChannelOutput<T>`。
- 删除 3 个复制粘贴的 channel runner + 4 个相同结构体（~150→~40 行）。
- `retrieval.rs`：提取 `apply_rerank_scores(chunks, score_by_chunk_id)` 消除 mm_reranker/reranker 分支重复。
- **验证：** `cargo test -p avrag_rag_core`。

### W5e — heavytail 清理（独立）
- `draft_sections`：引入 `struct DraftOptions { mpc, primed, one_sentence_per_line, persona, priming, on_section }` + `Default`，消除 12 参数 3 布尔。
- `bin/experiment.rs`（819 行）：将 `run_topic_arm_once`/`run_refine` 移入 library，bin 仅留 arg-parse + dispatch。4 臂复制粘贴 → 表循环。
- `fingerprint_workspace` 5× 复制 → `DraftWorkspace::fingerprint()` 单一方法。
- **验证：** `cargo test -p avrag_heavytail`。

### W5f — 其他去重（独立，小项）
- `llm/src/summary.rs`：提取 `LlmClient::complete_cached(messages, temperature, version_hash, cache)`。
- `app-billing/src/cost_events.rs`：3 个 recorder → 一个 `record_cost_event_if_available(auth, analytics, CostEvent)`。
- `pg_auth_store.rs`：读操作去掉不必要的事务包装；提取 `with_super_admin_tx` helper + `insert_legal_acceptance` helper。
- 删除 `auth` crate（2 行 re-export pass-through）或加文档说明存在理由。
- **验证：** 各自 crate 的 `cargo test`。

### M5 合并门
- `cargo test --workspace` + `cargo clippy --all-targets -- -D warnings` 全绿。
- 合并 `fix/tn-m5-dedup`。

---

## 推荐执行顺序与工作量估算

| 里程碑 | 工作量 | 风险 | 可并行度 | 产出 |
|--------|--------|------|----------|------|
| **M0** | 1 天 | 极低 | 4 subagent | ~2,000 行删除 |
| **M1** | 1 天 | 中 | 顺序 | 契约完整性恢复 |
| **M2** | 2-3 天 | 低-中 | 3 subagent | 前端去重 + 类型可靠 |
| **M3** | 4-7 天 | 高 | W3a 独立 + W3b/c 顺序 | 模块边界恢复 + 存储层拆 crate（解锁后续） |
| **M4** | 3-5 天 | 中-高 | 2-3 subagent | 业务逻辑归位 + memory store |
| **M5** | 2-3 天 | 低-中 | 5+ subagent | 长尾去重 |

**总计：~13-20 工作日**。可在任意里程碑后暂停评估。

**最小有效子集（如果只想先处理 CRITICAL）：** M0 + M1（2 天）即可消除死代码 + 修复契约漂移生产风险。
