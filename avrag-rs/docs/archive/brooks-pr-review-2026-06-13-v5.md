# Brooks-Lint Review

**Mode:** PR Review
**Scope:** 工作区未提交变更（222 文件，+3136/−13105）；采样覆盖 admin/billing/share 端口纵切、`contracts` 契约上移、`app-chat` agent loop、`llm`/`transport-http` 机械拆分、E2E 基建、前端 transport 收敛、CI workflow 根目录归并。**审查为采样模式**——变更体量超出单次 PR 可完整逐行评审范围。
**Health Score:** 63/100
**Trend:** 78 → 63（−15，对照 v4 报告；`.brooks-lint-history.json` 上一条 PR Review 亦为 78）

**一句话结论：** 这是一次方向正确的架构 consolidation（admin 删除、billing/share 端口化、`common` → `contracts` 契约上移、E2E 死管道已拆除、IPC 契约已补齐），但 Git 工作区处于「删旧未录新」的半完成态——41 个未跟踪替换目录/文件与 222 个已跟踪变更并存，是当前最大风险；在 staging 完整前不应合并。

---

## 变更概览

| 主题 | 规模（约） | 方向 |
|------|-----------|------|
| Admin 纵切 | 删除 `crates/admin`（166 行），`transport-http/routes/admin.rs` 改走 `AdminStorePort` | ✅ 正确：HTTP 不再直连 `PgAppRepository` |
| Billing 端口化 | 删除 `core_usage.rs`/`core_webhooks.rs`/`core_support.rs`（~1500 行），新增 `BillingStorePort` + `pg_billing_store` | ✅ DIP 方向正确 |
| Share 端口化 | `members.rs`/`public_read.rs`/`sharing.rs` 去 inline SQL，改 `ShareStorePort` | ✅ 与 admin 纵切一致 |
| 契约上移 | `common::{rag_execute,tool_call}` → `contracts`；全仓 import 批量替换 | ✅ 减少 `common` 上帝 crate 压力 |
| LLM client 拆分 | 删除 `llm/src/client.rs`（1262 行）→ `llm/src/client/{mod,request,stream_parser,types,rate_limit}.rs` | ✅ M13 机械拆分 |
| auth_secondary 拆分 | 删除 `auth_secondary.rs`（1040 行）→ `lib_impl/auth/{profile,preferences,reset}.rs` | ✅ 与 merged-fix-plan M13 对齐 |
| chat_private 拆分 | 删除 `chat_private.rs`（1122 行）→ `chat_private/{memory,quota,visibility,profile_*}.rs` | ⚠️ 新目录**未跟踪**（见 Critical） |
| storage-pg 拆分 | `repository_retrieval.rs` 部分移出 → `repository_retrieval_{lifecycle,cleanup}.rs` | ⚠️ 新文件**未跟踪** |
| E2E 基建 | `x-mock-rag-query` 死管道已拆除；`concurrent_query` 重命名 + `#[ignore]` real-LLM 变体；`e2e-gates.md` 记录 verified PASS | ✅ v4 三项 Warning 已响应 |
| 前端 transport | 新增 `lib/http/request.ts`；auth/admin/settings/dashboard/workspace/share 改走 `restRequest`；`tauri-ipc.ts` 补齐 AbortSignal/ApiError/body 校验；新增 `tests/runtime/transport.test.ts` | ✅ 方向正确，但未完成（见 Warning） |
| CI 归并 | 删除 `avrag-rs/.github/workflows/*` 与 `frontend_next/.github/workflows/*`；根目录 `.github/workflows/*` 新增（**未跟踪**） | ⚠️ 需与 Git staging 一并提交 |
| Agent loop | `iteration.rs` 重构 + 行数增至 **1147** | ⚠️ 认知负载仍在累积 |

### v4 已销项（不构成 finding）

- `x-mock-rag-query` 双端死管道：已按 M10 option b+c 删除；`rg 'x-mock-rag-query' product_e2e` = 0。
- `concurrent_query` 名实不符 + 孤儿断言：`concurrent_rag_queries_are_safe_on_codegen_bridge` 已重命名；`real_llm_concurrent_rag_queries_have_independent_citation_chunks`（`#[ignore]`）挂回 `assert_independent_citation_chunks`；`e2e-gates.md` 记录 mock 路径 **verified PASS**（2026-06-12, 20.5s）。
- IPC LSP/Hyrum 三项：`streamChatViaIPC` 已实现 `signal` abort + `chat_cancel`；`requestViaIPC` 已映射 `ApiError` 且非字符串 body 抛 `TypeError`；`transport.ts` 注释已声明 IPC 不支持自定义 headers。
- `cargo check --workspace` 本地通过；`pnpm vitest run tests/runtime/transport.test.ts` 4/4 通过。

---

## Findings

### 🔴 Critical

**Change Propagation — Git 工作区「删旧未录新」：41 个替换产物未跟踪，与 222 个已跟踪变更半脱节**

Symptom: `git status` 显示大量删除（`llm/src/client.rs`、`transport-http/.../auth_secondary.rs`、`app-chat/src/chat_private.rs`、`common/src/{rag_execute,tool_call}.rs` 等）同时存在 41 个 `??` 未跟踪路径，包括 `llm/src/client/`、`transport-http/src/lib_impl/auth/`、`app-chat/src/chat_private/`、`app-bootstrap/src/adapters/pg_{billing,share,usage_limit}_store.rs`、`contracts/src/{rag_execute,tool_call}.rs`、根目录 `.github/workflows/*.yml`、`frontend_next/lib/http/` 等。本地 `cargo check` 通过仅因磁盘上文件仍在；若只 `git add` 已修改文件并提交，克隆方/CI 将立即编译失败。

Source: Brooks — *The Mythical Man-Month*, Ch. 2（协调成本）；Feathers — *Working Effectively with Legacy Code*, Ch. 1（无完整变更集的提交等同无保护变更）

Consequence: 任何基于当前 diff 的 partial commit 或 PR 合并都会破坏主干；reviewer 看到的 diff 与可构建代码不一致，Health Score 与审查结论均不可信。

Remedy: 合并前执行 `git add` 补全所有替换目录；用 `git diff --stat HEAD` 确认「删旧 + 录新」成对出现；建议按 merged-fix-plan 的 M1/M2/M10/M13 纵切拆成 3–4 个可独立编译的 PR，而非一次 222 文件提交。

---

### 🟡 Warning

**Change Propagation — 222 文件 / −10k 行单体变更，超出单次可评审范围**

Symptom: 工作区 222 文件变更横跨 Rust 后端（admin/billing/share/chat/rag/llm/storage/transport）、`contracts`、`frontend_next` transport、CI workflow 迁移至少四条独立工作流；PR Review 指南对 >500 行变更要求采样并标记为 Change Propagation 信号。

Source: Brooks — *The Mythical Man-Month*, Ch. 2 Brooks's Law；Fowler — *Refactoring*, Shotgun Surgery

Consequence: 隐藏回归（端口 wiring 遗漏、import 漏改、桌面端路径断裂）无法在单次 review 中被发现；合并后故障定位需跨 10+ crate 追溯。

Remedy: 按 merged-fix-plan 依赖图拆分：M1 admin → M5 billing/share ports → M13 机械拆分 → M10 E2E → 前端 transport；每个 PR <50 文件且可独立 `cargo test` / `pnpm test`。

**Dependency Disorder — 前端 transport 迁移未完成：`billing`/`preferences`/`workspace/stream` 仍直连 `fetch`，桌面端路径断裂**

Symptom: `lib/http/request.ts` 与 `lib/runtime/transport.ts` 已建立统一入口，auth/admin/settings/dashboard/workspace/share 已迁移；但 `lib/billing/api.ts`（L87–105）、`lib/dashboard/preferences.ts`（L57）、`lib/workspace/stream.ts`（L415）仍各自实现 `decodeError` + 直接 `fetch(buildApiUrl(...))`，不经 `restRequest`。Tauri 桌面端走 IPC 时，billing 套餐/用量、偏好设置、以及任何仍调用 `workspace/stream` 内部 fetch 的路径将无法工作。

Source: Martin — *Clean Architecture*, DIP；Winters et al. — *Software Engineering at Google*, Hyrum's Law（错误类型与 fetch 语义是事实契约）

Consequence: transport 收敛到一半时，桌面壳发布即带 billing/偏好静默失败或错误类型不一致（`instanceof ApiError` 分支失效）；每新增一个未迁移 client 就扩大断裂面。

Remedy: 将 `billing/api.ts`、`dashboard/preferences.ts` 改为 `import { request } from "../http/request"`；`workspace/stream.ts` 的 SSE 路径评估是否需 IPC 对等实现或明确标注「Web-only」并在桌面端禁止调用；补 transport 契约测试覆盖 billing 错误映射。

**Cognitive Overload — `iteration.rs` 1147 行，核心方法仍超 50 行**

Symptom: 本轮 diff 对 `agents/loop/iteration.rs` 重构后文件达 **1147 行**；`apply_llm_output`（~211–378，~168 行）、`dispatch_native_tool_calls`（~100 行）、`dispatch_codegen`（~127 行）混合 LLM 输出解析、工具分发、codegen 桥接、telemetry 于同一 impl 块。同 crate 的 `chat_private/` 已按 memory/quota/visibility 拆分，loop 侧尚未对称完成。

Source: Fowler — *Refactoring*, Long Method；Ousterhout — *A Philosophy of Software Design*, Ch. 4 Deep Modules（接口复杂但隐藏不足）

Consequence: agent loop 是最高频变更区；单文件多职责使任何工具/策略改动都需要理解整文件上下文，回归测试难以精准定位，与 merged-fix-plan M2 目标（iteration 步骤提取）仍有差距。

Remedy: 按已有 `dispatch_tool_call`/`dispatch_codegen`/`dispatch_content` 边界继续提取子模块（如 `iteration_codegen.rs`、`iteration_tools.rs`）；目标单文件 <600 行、单函数 <40 行。

**Coverage Illusion — Billing/Share 端口已引入，但缺少与 admin 对等的 port contract 测试**

Symptom: `app-core` 新增 `BillingStorePort`（~165 行 trait）与 `ShareStorePort`（~140 行 trait），bootstrap 新增 `pg_billing_store.rs` / `pg_share_store.rs`（均**未跟踪**）；对比 admin 已有 `app-admin/tests/storage_port_contract.rs`（4 项）与 `admin_store_behavior.rs`，billing/share **零** dedicated port contract 测试。`cargo test -p app-admin --test storage_port_contract` 仅覆盖 admin escape-hatch 禁令。

Source: Feathers — *Working Effectively with Legacy Code*（无测试的 seam 不可安全变更）；Google — *How Google Tests Software*（change coverage）

Consequence: billing webhook 解析、share token 生命周期等 SQL 行为变更无编译期/行为契约约束；端口 trait 方法签名漂移只能在 integration/E2E 层暴露，反馈环路过长。

Remedy: 参照 `storage_port_contract.rs` 为 `BillingStorePort` / `ShareStorePort` 各增 memory-mode fake + 1–2 个行为测试（如 `get_share_settings` 映射、`get_current_subscription` 缺省 free plan）；PG adapter 留 integration 层验证。

---

### 🟢 Suggestion

**Knowledge Duplication — 前端仍有三份独立 `decodeError`，与 `http/request.ts` 的 `decodeApiError` 并行**

Symptom: `lib/http/request.ts` 已集中 `decodeApiError`（含 nested error envelope 解析）；`billing/api.ts`、`workspace/stream.ts`、`dashboard/preferences.ts` 仍保留简化版本地 `decodeError`（billing 版甚至不处理 nested `{ error: { message } }` 形态，与 centralized 版行为不一致）。

Source: Hunt & Thomas — *The Pragmatic Programmer*, DRY；Fowler — *Refactoring*, Duplicate Code

Consequence: 错误文案/状态码解析逻辑漂移；billing 路径对嵌套 envelope 的降级行为与其他 client 不同。

Remedy: 三处改为 `import { decodeApiError } from "../http/request"` 或统一走 `request()`/`fetchResponse()`，删除本地副本。

**Accidental Complexity — `chat_private/profile_merge.rs` 提取后遗留 3 个未使用函数**

Symptom: `cargo check -p app-chat` 报告 `apply_singleton_update`、`apply_hint_updates`、`apply_profile_delta_from_value` 为 `dead_code`（profile_merge.rs:467/477/493）；M3 typed 化拆分进行中，旧 JSON 操纵路径尚未清理或接线。

Source: Fowler — *Refactoring*, Lazy Class / Dead Code

Consequence: 读者无法判断这些函数是「即将使用」还是「应删除」；增加 M3 完成度的误判成本。

Remedy: 若 M3 测试已覆盖 typed 路径，删除三函数；否则补测试并接线，去掉 `dead_code` 警告。

**Recommended fix order:** Git staging 完整性（Critical）→ PR 拆分（Warning #2）→ 前端 transport 收尾（Warning #3）→ billing/share port tests（Warning #4）→ iteration 继续拆分（Warning #5）→ 三个 Suggestion 顺手清理。

---

## Summary

本轮变更的架构方向值得肯定：admin 旧 crate 删除、billing/share 端口化、`contracts` 承接跨 crate 契约、E2E 死管道与 IPC 契约问题已在工作区修复，且 v4 多项 Warning 已落地。当前最大 blocker 不是设计，而是 **Git 工作区完整性**——41 个未跟踪替换文件必须与删除项同批提交，否则审查与 CI 结论均无效。其次应将 222 文件大 diff 按 merged-fix-plan 纵切拆分，并在前端 transport 迁移完成前避免扩大桌面端发布面。建议合并前跑一轮 `E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1` 确认端口化未破坏 mock 套件；billing/share 端口落地后考虑 `/brooks-lint:brooks-test` 补测 seam 契约。

---

*报告生成：2026-06-13 · Brooks-Lint PR Review v5 · 上一版报告已归档至 `docs/archive/brooks-pr-review-2026-06-12-v4.md`*
