# 延迟项与长尾项详细计划 — Thermo-Nuclear Review S2 后续

> 基于 `THERMO_NUCLEAR_REVIEW_2026-07-08_S2.md` 审查 + M0-M5 执行结果。
> 当前分支链：master ← m0 ← m1 ← m2 ← m0.5 ← m2.5 ← m3 ← m4 ← m5
> 23 个提交，整个 workspace 编译通过。
> 本文档覆盖所有 **未完成** 的审查发现。

---

## 已完成清单（23 提交）

| 里程碑 | 提交 | 内容 |
|--------|------|------|
| M0 | `679d7de` | 13k 行死代码删除 |
| M1 | `eab3704` `0561321` `fc356f4` | F1 CRITICAL 契约漂移修复 + 金标准测试 + 死配置清理 |
| M2 | `38fba54` `565ba5a` `99f66b9` `fa9555b` `6a1408e` | ApiEnvelope 统一 + mapper 统一 + zod + DOMPurify + string utils |
| M0.5 | `4e63332` | docker rm 修复 + outcomes.rs 删除 + dev gate + Write arm |
| M2.5 | `f50c237` | zod discriminatedUnion + CSS module 修复 |
| M3 | `b0b8cf3` `5c020b1` `c59dbde` `c23f90b` | 25 include!→mod + PgAppRepository 99方法→9结构体 |
| M4 | `1134d37` `be9a732` `67a0fca` | AUTH bypass 清理 + PasswordResetService + BillingService + Memory*Store(-322行) |
| M5 | `515e039` `17aa99f` `bf1df28` `e571dbc` `6ce178c` | IngestionError typed + DraftOptions + Pipeline(-128行) + RunContext + Desktop(-175行) |

---

## 第一部分：延迟项（已计划但未执行）

### DEFERRED-1. W4c — ShareService 提取 [MEDIUM]

**发现：** NEW-4 (MEDIUM)
**位置：** `avrag-rs/crates/transport-http/src/handlers/notebooks/share.rs`（600 行）
**问题：** 10 个 handler 内联了完整的访问控制 + 业务逻辑（create/revoke/get/update/validate/access-level/analytics/logs/token/api-keys）。与 S3 reset.rs / billing/api.rs 同一模式。
**为何延迟：** W4a/W4b 优先完成（更高优先级）。Share handler 的业务逻辑不如 password-reset/billing 复杂（主要是 CRUD + 权限检查），提取 Service 的收益相对较低。
**工作量：** 1 天
**风险：** 中

**计划：**
1. 创建 `crates/app-bootstrap/src/services/share.rs`：`ShareService` struct，持有 share_store 引用
2. 将 10 个 handler 的核心逻辑移入 service 方法
3. Handler 变为 thin controller（权限检查 + 参数提取 → service 调用 → 响应格式化）
4. 验证：`cargo test -p transport-http`

---

### DEFERRED-2. W4e — 分解 StorageContext [CRITICAL → 高风险延迟]

**发现：** B4 (CRITICAL)
**位置：** `avrag-rs/crates/app-core/src/storage_context.rs:25`
**问题：** 19 字段 god-bag（stores + caches + config + object-store 设置混在一起），18-positional-arg 构造器。113 处引用该类型。
**为何延迟：** 113 处引用使分解风险极高。需要同时修改 app-core、app-bootstrap、transport-http、app-documents、app-admin、app-chat 等几乎所有 crate。必须在 W3c 结构体拆分完全稳定后再做。
**工作量：** 3-5 天
**风险：** 极高

**计划：**
1. 将 19 字段分为 3 组：
   - `StorageStores`：document_store, auth_store, admin_store, billing_quota, billing_store, share_store, chat_persistence（7 个 domain store）
   - `StorageInfra`：postgres_health, postgres_configured, uses_memory_adapters, max_upload_file_size_bytes（4 个 infra 字段）
   - `ObjectStoreConfig`：object_store, public_base_url, object_root, upload_expire_sec, download_expire_sec（5 个 object store 字段）
   - `MemoryState`：inner, api_keys, api_key_hashes（3 个 in-memory 状态）
2. `StorageContext` 变为 `StorageStores + StorageInfra + ObjectStoreConfig + MemoryState` 的组合
3. 构造器从 18 positional params 改为 builder 模式或 struct literal
4. 逐 crate 更新所有引用（每次 `cargo build --workspace` 验证）
5. **前置条件：** 所有 M3 拆分已稳定，workspace test 全绿

---

### DEFERRED-3. W05e — 删除 auth crate pass-through [MEDIUM]

**发现：** E7 (MEDIUM)
**位置：** `avrag-rs/crates/auth/src/lib.rs`（2 行 `pub use contracts::auth_runtime::*`）
**为何延迟：** 需要修改 ~20 个文件的 `use avrag_auth::` → `use contracts::auth_runtime::`，以及 10 个 Cargo.toml。最好与 M3 的后续改动合并做。
**工作量：** 0.5 天
**风险：** 低

**计划：**
1. 在所有 dependent crate 的 Cargo.toml 中移除 `avrag-auth` 依赖，添加 `contracts` 依赖（如果还没有）
2. 批量替换 `use avrag_auth::` → `use contracts::auth_runtime::`
3. 从 workspace Cargo.toml 移除 `auth` member
4. 删除 `crates/auth/` 目录
5. 验证：`cargo build --workspace`

---

### DEFERRED-4. W2b — useSessionMessages hook [MEDIUM]

**发现：** M2-S2 (MEDIUM)
**位置：** `frontend_next/components/workspace/workspace-history-pane.tsx`
**问题：** `listWorkspaceSessionMessages` 在 2 个独立 useEffect 中为同一 session 各拉一次消息列表（title 派生 + 搜索文档）。
**为何延迟：** 需要 React hooks 架构知识，subagent 尝试失败。
**工作量：** 0.5 天
**风险：** 低-中

**计划：**
1. 创建 `frontend_next/hooks/use-session-messages.ts`
2. 接口：`useSessionMessages(token, sessions, enabled) → { messagesBySession, loading }`
3. 内部用 `useRef<Set<string>>` 跟踪已请求的 session ID，避免重复请求
4. 修改 history-pane 的 2 个 useEffect 从 hook 读取
5. 保留 title-sync 逻辑（向服务器更新派生标题）不动
6. 验证：`pnpm typecheck && pnpm test`

---

## 第二部分：长尾项（M5 剩余）

### LONGTAIL-1. W5e — 统一 dispatch 提取 [MEDIUM]

**发现：** A3 (MEDIUM)
**位置：** `avrag-rs/crates/app-chat/src/agents/unified/mod.rs`（365 行）
**问题：** Chat/Rag/Search 三臂 dispatch 各 ~50-75 行，重复 mode-config-load + LLM-unavailable 检查 + `with_observer` + `ReActLoop::new` + `run`。
**前置依赖：** W5c RunContext（已完成）
**工作量：** 1 天
**风险：** 中

**计划：**
1. 提取 `run_react_mode(mode_id, llm_resolver, extras, request, sink, ctx) -> Result<AgentRunResult>`
2. 三臂变为 3 次调用，差异通过 `llm_resolver` 闭包和 `extras` 配置传入
3. ~120 行 → ~30 行
4. 验证：`cargo test -p app-chat`

---

### LONGTAIL-2. W5f — retired-skills 数据驱动 [MEDIUM]

**发现：** A5 (MEDIUM)
**位置：** `avrag-rs/crates/app-chat/src/agents/capability/registry.rs:133-157`
**问题：** 20-entry 硬编码 `matches!` denylist。
**工作量：** 0.5 天
**风险：** 低

**计划：**
1. 将 denylist 移到 `modes/` YAML 或 `skills/deprecated.toml`
2. 启动时加载，`is_retired_skill()` 从数据读取
3. 或者利用已有的 `deprecation` frontmatter 机制（如果 skills 有 frontmatter）
4. 验证：`cargo test -p app-chat`

---

### LONGTAIL-3. W5g — 拆 ChatPersistencePort [HIGH]

**发现：** C3 (HIGH)
**位置：** `avrag-rs/crates/rag-core-ports/src/chat_persistence.rs`（22 方法）
**问题：** Kitchen-sink trait 混合 notebook CRUD、session CRUD、message CRUD、user profile、conversation-history search、notifications、usage events、document assets、multimodal chunks、audit records、chunk retrieval、summary metadata。违反接口隔离原则（ISP）。
**前置依赖：** M3 W3c 已完成（域结构体已拆分）
**工作量：** 2-3 天
**风险：** 高（跨 3 crate 接口变更）

**计划：**
1. 将 22 方法按域拆分为聚焦 traits：
   - `SessionPort`：create_session, get_session, update_session, delete_session, list_sessions, search_sessions
   - `MessagePort`：get_message, append_chat_turn, search_conversation_history
   - `ProfilePort`：get_user_profile, upsert_user_profile
   - `NotificationPort`：create_notification, list_notifications, mark_notification_read, create_notifications_for_all_users
   - `UsagePort`：record_usage_event
   - `ApiKeyPort`：create_api_key, list_api_keys, revoke_api_key, validate_api_key
   - `AuditPort`：append_audit_record, prune_audit_log
2. `PgChatPersistenceAdapter`（app-bootstrap）实现多个 trait
3. 更新所有 `ChatPersistencePort` 引用为具体 trait
4. 验证：`cargo test --workspace`

---

### LONGTAIL-4. W5h — 拆 RetrievalDataPlane [MEDIUM]

**发现：** C4 (MEDIUM)
**位置：** `avrag-rs/crates/retrieval-data-plane/src/lib.rs`（21 行代码，9 方法，4 个返回 `Err("not implemented")`）
**问题：** Fat trait 混合 read + write + schema，带运行时 panic stubs。
**工作量：** 1 天
**风险：** 中

**计划：**
1. 拆为 `RetrievalReadPort`（search 系列方法）+ `RetrievalIndexPort`（replace/delete/ensure_schema）
2. Milvus adapter 只实现它支持的 trait
3. 验证：`cargo test --workspace`

---

### LONGTAIL-5. W5i — 删除 pass-through adapters + 统一 IndexedChunk [MEDIUM]

**发现：** C2 (MEDIUM)
**位置：** `avrag-rs/crates/app-bootstrap/src/adapters/pg_content_store.rs`（124 行），`pg_chat_persistence.rs`（315 行）
**问题：** 纯 delegation adapter + `IndexedChunk` 定义 3 次（2 live + 1 dead）。
**前置依赖：** M3 W3c 已完成
**工作量：** 1 天
**风险：** 中

**计划：**
1. 检查 M3c 拆分后 pass-through adapter 是否仍需要（如果 caller 可以直接用域结构体，删除 adapter）
2. 统一 `IndexedChunk` 定义为 1 个（保留 `common/src/content_store.rs` 版本，删除 `storage-pg/src/lib_impl/errors_and_mappers.rs` 中的版本）
3. 更新所有引用
4. 验证：`cargo test --workspace`

---

### LONGTAIL-6. W5j — 统一 analytics 路径 [HIGH]

**发现：** E5 (HIGH)
**位置：** 4 个独立实现：
- `transport-http/src/lib_impl/router_core.rs:78` — `record_api_product_event_if_available`（free function，6 调用点）
- `app-bootstrap/src/app_state/state_methods.rs:162` — `AppState::record_product_event_if_available`（方法，33 调用点）
- `app-documents/src/analytics_helpers.rs:6` — `record_product_event_if_available`（app-documents 版本）
- `app-chat/src/context.rs:75` — `record_product_event_if_available`（app-chat 版本）

**问题：** 同一概念 4 个实现，必然 drift。free function 硬编码 `Surface::Api` + `client_platform:"web"`。
**工作量：** 1 天
**风险：** 中

**计划：**
1. 以 `AppState::record_product_event_if_available` 为单一入口
2. `record_api_product_event_if_available` 改为内部调用 `state.record_product_event_if_available(event_name, Surface::Api, result, None, None, metadata)`
3. app-documents 和 app-chat 版本检查是否能统一到同一 trait
4. 如果某些场景需要不同 surface（如 `Surface::Worker`），通过参数传入而非硬编码
5. 验证：`cargo test --workspace`

---

### LONGTAIL-7. W5k — rag_execute.rs 层理 [HIGH]

**发现：** F5 (HIGH)
**位置：** `contracts/src/rag_execute.rs`（626 行）
**问题：** Contract crate 托管了运行时逻辑（验证引擎 9 个错误变体 + retrieval-prep 策略 reorder/truncate + graph-triplet 分类 fuzzy/traceable/resolved + compat conversion）。Contract crate 应该只有 wire shapes。
**额外问题：** doc comment 说 "replaces to_chat_request_compat()" 但该函数仍存在且被调用——误导。
**工作量：** 2 天
**风险：** 中-高

**计划：**
1. 将 `ExecutePlanRequest::validate()`（65 行验证逻辑）移到 `rag-core/src/validation/` 或 `app-chat/src/rag_execute/`
2. 将 `ensure_original_query_text_dense_item()`（retrieval-prep 策略）移到 rag-core
3. 将 `PlaceholderTriplet::classify()`（graph 分类）移到 rag-core 或 retrieval-data-plane
4. `from_rag_plan()` 和 `to_chat_request_compat()`（compat 转换）评估是否可以删除（如果不再需要）
5. Contract crate 的 `rag_execute.rs` 只保留类型定义（wire shapes）
6. 修复误导性 doc comment
7. 验证：`cargo test -p contracts && cargo test --workspace`

---

### LONGTAIL-8. W5l — pg_auth_store 去事务包装 [MEDIUM]

**发现：** B6 (MEDIUM)
**位置：** `avrag-rs/crates/storage-pg/src/lib_impl/repository_auth_user.rs`（335 行）
**问题：** 22 个 `pool.begin/tx.commit`，其中只读 SELECT 不需要事务包装。
**工作量：** 0.5 天
**风险：** 低

**计划：**
1. 识别只读方法（`list_api_keys`, `get_user_profile`, `validate_api_key` 等）
2. 将只读方法的 `begin → SELECT → commit` 改为直接 `pool.query`
3. 保留写操作的事务包装
4. 提取 `with_super_admin_tx` helper（写操作用）
5. 验证：`cargo test -p avrag-storage-pg`

---

### LONGTAIL-9. D5 — processor.rs god function [HIGH]

**发现：** D5 (HIGH)
**位置：** `avrag-rs/bins/worker/src/pipeline/processor.rs`（`process()` ~377 行）
**问题：** 整个 body 在一个 `tokio::time::timeout` 中，内联锁获取（Redis + PG advisory）、payload dispatch、URL fetching、pipeline 调用、telemetry、错误处理。每任务克隆所有 LLM/embedding client。
**工作量：** 2 天
**风险：** 中-高

**计划：**
1. 提取锁获取为独立函数 `acquire_task_lock`
2. 提取 payload dispatch 为独立函数 `dispatch_payload`
3. 提取 finish-run 分支为独立函数
4. 将 client 克隆移到构造时（而非每任务）
5. 主 `process()` 变为 ~100 行的编排函数
6. 验证：`cargo test -p avrag-worker`

---

### LONGTAIL-10. NEW-5 — 修复 WIP admin 组件 i18n [CRITICAL]

**发现：** NEW-5 (CRITICAL — 102→96 typecheck errors)
**位置：** `frontend_next/components/admin/admin-*.tsx`（未跟踪 WIP 文件）
**问题：** WIP admin 组件调用 `adminMessage(locale, "common.loading")` 等 i18n key，但这些 key 不存在于任何 message 文件。两套并行 i18n 系统（`INLINE_COPY` table in `admin-i18n.ts` vs `UI_MESSAGES` in `lib/i18n/messages/`）。
**注意：** 这些是未跟踪的 WIP 文件，可能是 WIP 开发者的责任。
**工作量：** 1 天
**风险：** 低（只改 WIP 文件）

**计划：**
1. 确认这些组件是否应在本次修复范围内（询问 WIP 开发者）
2. 方案 A：添加缺失的 i18n keys 到 `lib/i18n/messages/common.ts`（新建 `organizations.ts`, `organizationDetail.ts`）
3. 方案 B：将 admin surfaces 改用 `adminText`/`INLINE_COPY` 系统
4. 方案 C：统一两套 i18n 为一套
5. 验证：`pnpm typecheck` → 0 errors

---

## 优先级排序与建议执行顺序

### Phase 1: 高杠杆低风险（~3 天）
1. **LONGTAIL-8** (W5l pg_auth_store 去事务) — 0.5 天，低风险
2. **DEFERRED-3** (W05e 删除 auth crate) — 0.5 天，低风险
3. **LONGTAIL-2** (W5f retired-skills) — 0.5 天，低风险
4. **DEFERRED-4** (W2b useSessionMessages hook) — 0.5 天，低-中
5. **DEFERRED-1** (W4c ShareService) — 1 天，中风险

### Phase 2: 中杠杆中风险（~4 天）
6. **LONGTAIL-6** (W5j 统一 analytics) — 1 天，中风险
7. **LONGTAIL-1** (W5e 统一 dispatch) — 1 天，中风险
8. **LONGTAIL-5** (W5i 删除 pass-through adapters) — 1 天，中风险
9. **LONGTAIL-4** (W5h 拆 RetrievalDataPlane) — 1 天，中风险

### Phase 3: 高杠杆高风险（~7 天）
10. **LONGTAIL-3** (W5g 拆 ChatPersistencePort) — 2-3 天，高风险
11. **LONGTAIL-7** (W5k rag_execute 层理) — 2 天，中-高风险
12. **LONGTAIL-9** (D5 processor.rs) — 2 天，中-高风险

### Phase 4: 最高风险（~5 天）
13. **DEFERRED-2** (W4e 分解 StorageContext) — 3-5 天，极高风险

### 独立项
14. **LONGTAIL-10** (NEW-5 admin i18n) — 需确认是否在范围内

**总计：~19 工作日**（如果全部执行）。建议按 Phase 顺序推进，每个 Phase 完成后评估。

---

## 验证门规范

每个 workstream 完成后执行：

```bash
# Rust 后端（在 avrag-rs/）
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy --all-targets -- -D warnings

# 前端（在 frontend_next/）
pnpm typecheck && pnpm test

# 契约 codegen 漂移
pnpm generate:contracts && git -C .. diff --exit-code -- frontend_next/lib/contracts/generated/

# Desktop（如果修改了 desktop/）
cd desktop/src-tauri && cargo build
```
