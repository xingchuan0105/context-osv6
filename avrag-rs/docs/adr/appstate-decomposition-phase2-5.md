# ADR: AppState 拆分 Phase 2–5

> 状态：Draft
> 日期：2026-06-11
> 前置：Phase 1（AnalyticsContext）已完成，commit 3316651

---

## 1. 背景

AppState 是 app crate 的核心结构体，当前有 22 个字段、15 个 impl 块（跨 11 个文件）、47 个文件引用它。它承担了认证、存储、LLM、RAG、编排、分析、计费、对象存储、限流的全部职责，是一个典型的 God Object。

Phase 1 已将 analytics 相关字段和方法提取为独立的 `AnalyticsContext`，旧方法保留为委托。Phase 2–5 继续按依赖关系渐进式拆分。

### 当前 AppState 字段（22 个）

```
auth              → AuthContext (已提取为独立 crate)
pg                → STORAGE
inner             → STORAGE (memory-mode)
llm_client        → LLM
memory_llm_client → LLM
chatmemory        → ORCHESTRATION
analytics         → ANALYTICS (Phase 1 已提取)
quota_manager     → BILLING
rag_runtime       → RAG
agent_service     → ORCHESTRATION
object_store      → OBJECT STORAGE
guard_pipeline    → GUARDRAILS
uses_memory_adapters → STORAGE flag
public_base_url   → OBJECT STORAGE / CONFIG
object_root       → OBJECT STORAGE / CONFIG
usage_limit_phase → BILLING / CONFIG
search_provider   → SEARCH / CONFIG
search_mode       → SEARCH / CONFIG
redis_url         → CONFIG
object_storage_upload_expire_sec   → OBJECT STORAGE / CONFIG
object_storage_download_expire_sec → OBJECT STORAGE / CONFIG
max_upload_file_size_bytes         → STORAGE / CONFIG
api_keys          → AUTH / STORAGE
key_vault         → AUTH / SECURITY
```

### 关键架构洞察

- `auth` 是 **per-request** 的（中间件通过 `state.with_auth(auth)` 创建克隆），其余是 **per-app-lifetime** 的
- AppState 已 derive `Clone`（所有重型依赖用 Arc 包装），通过值传递（克隆）而非 `Arc<AppState>`
- `AuthContext` 已是独立 crate（`avrag_auth`），AppState 存储它作为字段，通过 `with_auth()` 做 per-request 替换

### 跨域依赖热力图

| 字段 | auth | pg | analytics | quota | guard | chatmemory |
|------|:----:|:--:|:---------:|:-----:|:-----:|:----------:|
| pg CRUD 方法 | ✅ | — | ✅ | ✅ | | |
| agent/chat 方法 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| infer_profile_delta | ✅ | ✅ | | | ✅ | ✅ |
| llm_client | | | | | | |
| object_store | ✅ | | ✅ | | | |
| rag_runtime | ✅ | ✅ | | | | |

---

## 2. Phase 2: 提取 LlmContext

### 目标

将 `llm_client`、`memory_llm_client` + 温度配置从 AppState 移出。

### 受影响字段

```rust
llm_client: Option<LlmClient>,
memory_llm_client: Option<LlmClient>,
```

### 受影响方法

| 方法 | 当前位置 | 依赖 |
|------|---------|------|
| `memory_llm_temperature()` | state_methods.rs | 无（硬编码常量） |
| `agent_llm_temperature()` | state_methods.rs | 无（硬编码常量） |
| `infer_profile_delta()` | state_methods.rs | auth, pg, guard, chatmemory, llm_client |

`llm_client` 和 `memory_llm_client` 在 agent 构造和 dream layer（profile inference）中使用。agent runtime 通过 `UnifiedAgentService` 间接使用 LLM，不直接访问 AppState 的 llm_client。

### 设计

```rust
// crates/app/src/llm_context.rs
#[derive(Clone)]
pub struct LlmContext {
    pub llm_client: Option<LlmClient>,
    pub memory_llm_client: Option<LlmClient>,
}

impl LlmContext {
    pub fn new(
        llm_client: Option<LlmClient>,
        memory_llm_client: Option<LlmClient>,
    ) -> Self { ... }

    pub fn memory_llm_temperature(&self) -> f32 { 0.3 }
    pub fn agent_llm_temperature(&self) -> f32 { 0.7 }

    /// LLM client for general agent tasks
    pub fn agent_client(&self) -> Option<&LlmClient> { self.llm_client.as_ref() }

    /// LLM client for dream layer / memory tasks
    pub fn memory_client(&self) -> Option<&LlmClient> {
        self.memory_llm_client.as_ref().or(self.llm_client.as_ref())
    }
}
```

### AppState 改动

```rust
// state_types.rs
pub(crate) llm_ctx: LlmContext,  // 替代 llm_client + memory_llm_client

// state_methods.rs
pub fn llm_ctx(&self) -> &LlmContext { &self.llm_ctx }
```

### `infer_profile_delta` 迁移

这是唯一一个深度使用 LLM client 的 AppState 方法。方案：

1. `infer_profile_delta` 改为接收 `&LlmContext` 参数（而非从 self 取）
2. 或将其移到 `MemoryContext`（Phase 2+ 与 chatmemory 一起处理）

推荐方案 1（最小改动）：签名改为 `infer_profile_delta(&self, llm: &LlmContext, ...)`，内部用 `llm.memory_client()` 取 client。

### 调用方影响

约 5 个直接调用点。`infer_profile_delta` 在 `chat_private.rs` 和 `service_postprocess.rs` 中调用。`memory_llm_temperature` / `agent_llm_temperature` 在 agent 构造中使用。

### 风险

极低。LLM client 的使用边界非常清晰，没有跨域耦合。

---

## 3. Phase 3: 提取 ObjectStorageContext

### 目标

将对象存储相关字段和方法从 AppState 移出。

### 受影响字段

```rust
object_store: Arc<ObjectStoreHandle>,
public_base_url: String,
object_root: String,
object_storage_upload_expire_sec: u64,
object_storage_download_expire_sec: u64,
```

### 受影响方法

| 方法 | 依赖 |
|------|------|
| `signed_upload_url()` | object_store, auth |
| `verify_upload_signature()` | key_vault |
| `object_root_path()` | object_root |
| `resolve_citation_asset_url()` | public_base_url, object_root |
| `get_citation_asset()` | object_store, pg |
| `lookup_citation()` | pg |

### 设计

```rust
// crates/app/src/object_storage_context.rs
#[derive(Clone)]
pub struct ObjectStorageContext {
    object_store: Arc<ObjectStoreHandle>,
    public_base_url: String,
    object_root: String,
    upload_expire_sec: u64,
    download_expire_sec: u64,
}

impl ObjectStorageContext {
    pub fn new(
        object_store: Arc<ObjectStoreHandle>,
        public_base_url: String,
        object_root: String,
        upload_expire_sec: u64,
        download_expire_sec: u64,
    ) -> Self { ... }

    pub fn signed_upload_url(&self, ...) -> ... { ... }
    pub fn object_root_path(&self) -> &str { &self.object_root }
    pub fn resolve_citation_asset_url(&self, ...) -> String { ... }
}
```

### 难点

`get_citation_asset()` 和 `lookup_citation()` 需要 pg 访问。方案：
- 接收 `&PgAppRepository` 作为参数
- 或保留在 AppState 上作为委托方法（调用方传 pg）

推荐：这两个方法保留在 AppState 上，只提取纯对象存储方法。

### 调用方影响

约 10 个调用点，集中在 document upload 和 citation 解析。

### 风险

低。对象存储边界相对清晰。`signed_upload_url` 需要 auth 信息（用于签名），传参即可。

---

## 4. Phase 4: 提取 BillingContext

### 目标

将计费/配额相关字段和方法从 AppState 移出。依赖 Phase 1（AnalyticsContext）。

### 受影响字段

```rust
quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
usage_limit_phase: String,
```

### 受影响方法

| 方法 | 依赖 |
|------|------|
| `get_user_usage_limit()` | quota_manager, auth |
| `check_user_quota()` | quota_manager, auth |
| `ensure_metric_quota()` | quota_manager, auth |
| `record_llm_usage_if_available()` | quota_manager, auth, analytics |

### 设计

```rust
// crates/app/src/billing_context.rs
#[derive(Clone)]
pub struct BillingContext {
    quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    usage_limit_phase: String,
}

impl BillingContext {
    pub fn new(
        quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
        usage_limit_phase: String,
    ) -> Self { ... }

    pub async fn get_user_usage_limit(&self, auth: &AuthContext) -> ... { ... }
    pub async fn check_user_quota(&self, auth: &AuthContext) -> ... { ... }
    pub async fn ensure_metric_quota(&self, auth: &AuthContext, ...) -> ... { ... }

    /// Record LLM usage into both billing metering AND analytics cost events.
    pub async fn record_llm_usage(
        &self,
        auth: &AuthContext,
        analytics: &AnalyticsContext,
        feature: BillableFeature,
        stage: &str,
        usage: &LlmUsage,
        source: &str,
    ) { ... }
}
```

### `record_llm_usage_if_available` 拆分

当前该方法同时调用 `quota_manager.rolling_service().record_usage()` 和 `self.record_cost_event_if_available()`。拆分后：

```rust
// BillingContext::record_llm_usage 内部：
// 1. billing: quota_manager.rolling_service().record_usage(...)
// 2. analytics: analytics_ctx.record_cost_event(...)
```

两步都通过参数获取，不再隐式依赖 AppState。

### 调用方影响

约 8 个调用点。`ensure_metric_quota` 在 chat preflight 和 document upload 中调用。`record_llm_usage_if_available` 在 chat pipeline 中调用。

### 风险

中。quota 检查嵌入在业务逻辑深处，需要仔细确认每个调用点的 auth 来源。

---

## 5. Phase 5: 分离 Storage 与 Orchestration

### 目标

将剩余的 AppState 字段拆为 `StorageContext`（数据持久化）和 `OrchestratorContext`（chat pipeline 编排）。这是最大、最复杂的阶段。

### 5.1 StorageContext

#### 字段

```rust
pub struct StorageContext {
    pg: Option<Arc<PgAppRepository>>,
    inner: Arc<RwLock<MemoryState>>,  // memory-mode fallback
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    max_upload_file_size_bytes: u64,
    uses_memory_adapters: bool,
}
```

#### 方法（约 30 个 CRUD 方法）

```
pg(), pg_ready(), runtime_mode()
list/create/update/delete notebooks, sessions, messages, documents
list_ready_documents_for_chat, search
load/save user_preferences, current_user_preferences
list/create/revoke api_keys
list/mark notifications, emit_notification
record_usage
enqueue_ingest_task, enqueue_reindex_task
memory_session_visible
```

#### 难点

几乎所有 CRUD 方法都引用 `self.auth`（传给 pg repository）和调用 analytics/billing（Phase 1/4 已解耦）。迁移策略：

1. 方法签名加 `auth: &AuthContext` 参数
2. analytics 调用改为 `analytics_ctx: &AnalyticsContext`
3. billing 调用改为 `billing_ctx: &BillingContext`

```rust
impl StorageContext {
    pub async fn create_notebook(
        &self,
        auth: &AuthContext,
        analytics: &AnalyticsContext,
        name: &str,
    ) -> Result<Notebook> {
        // ... pg.create_notebook(auth, name).await
        // ... analytics.record_product_event(...)
    }
}
```

### 5.2 OrchestratorContext

#### 字段

```rust
pub struct OrchestratorContext {
    agent_service: Option<Arc<UnifiedAgentService>>,
    chatmemory: Option<Arc<ChatMemory>>,
    guard_pipeline: Arc<GuardPipeline>,
}
```

#### 方法（约 15 个 chat pipeline 方法）

```
agent_service(), set_agent_service()
execute_chat(), execute_chat_stream()
execute_chat_pipeline(), execute_chat_preflight()
resolve_chat_session(), resolve_agent_messages()
build_agent_request(), build_general_agent_debug()
execute_clarify_mode_core(), execute_memory_chat_compat()
apply_output_guard_to_execution()
persist_chat_execution()
record_usage_for_execution()
emit_notifications_for_execution()
maybe_update_structured_profile(), infer_profile_delta()
remember_explicit_agent_preference(), delete_current_agent_preference()
```

#### 难点

这些方法使用 **全部** 子上下文：auth、pg、analytics、billing、llm、guard、chatmemory、rag。迁移策略：

```rust
impl OrchestratorContext {
    pub async fn execute_chat(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsContext,
        billing: &BillingContext,
        llm: &LlmContext,
        request: &ChatRequest,
    ) -> Result<ChatExecution> { ... }
}
```

这会导致方法签名非常长。缓解方案：

1. **ServiceBundle 模式**：创建一个轻量的 `ServiceBundle` 聚合所有子上下文
   ```rust
   pub struct ServiceBundle<'a> {
       pub auth: &'a AuthContext,
       pub storage: &'a StorageContext,
       pub analytics: &'a AnalyticsContext,
       pub billing: &'a BillingContext,
       pub llm: &'a LlmContext,
       pub rag: &'a RagContext,
       pub object_storage: &'a ObjectStorageContext,
   }
   ```
2. 方法接收 `&ServiceBundle` 而非 7 个独立参数

### 5.3 AppState 终态

Phase 5 完成后，AppState 变为薄壳：

```rust
pub struct AppState {
    pub(crate) auth: AuthContext,               // per-request, 由中间件替换
    pub(crate) storage: StorageContext,          // per-app
    pub(crate) orchestrator: OrchestratorContext, // per-app
    pub(crate) analytics: AnalyticsContext,      // per-app (Phase 1)
    pub(crate) billing: BillingContext,           // per-app (Phase 4)
    pub(crate) llm: LlmContext,                  // per-app (Phase 2)
    pub(crate) rag: RagContext,                  // per-app
    pub(crate) object_storage: ObjectStorageContext, // per-app (Phase 3)
    // config 字段（search_provider 等可归入对应 context）
}
```

保留 `with_auth()` 方法用于 per-request 克隆。

### 5.4 迁移策略

不一次性迁移所有 47 个文件。按子模块分批：

1. **子 PR 5a**：提取 StorageContext 的 notebook/session/document CRUD（~15 个方法）
2. **子 PR 5b**：提取 StorageContext 的 user_preferences/notification/api_keys 方法
3. **子 PR 5c**：提取 OrchestratorContext 的 chat pipeline 方法
4. **子 PR 5d**：提取 OrchestratorContext 的 dream layer 方法
5. **子 PR 5e**：清理 AppState 薄壳 + 更新 transport-http 的所有 handler

每个子 PR 独立编译、独立测试。

### 5.5 transport-http 适配

`transport-http` crate 的 handler 目前通过 `State(state): State<AppState>` 或 `Extension(RequestState(state))` 获取 AppState。Phase 5 后有两个选择：

**选择 A（推荐）**：AppState 保留为 handler 的入口，但 handler 内部通过 `state.storage()`、`state.orchestrator()` 等获取子上下文。不改 handler 签名。

**选择 B**：handler 改为接收 `State<OrchestratorContext>` + `Extension<StorageContext>`。改动量大但更干净。

推荐选择 A：Phase 5 只改 AppState 内部结构，不改外部 API。

### 调用方影响

47 个文件。按子 PR 分批，每批 ~10 个文件。

### 风险

高。需要：
- 仔细处理 auth 传递（参数 vs 嵌入）
- 确保每个子 PR 独立可编译
- 运行全量 smoke 测试验证

---

## 6. 总体时间线建议

| Phase | 内容 | 预估工作量 | 前置 |
|-------|------|-----------|------|
| ~~1~~ | ~~AnalyticsContext~~ | ~~已完成~~ | — |
| 2 | LlmContext | 0.5 天 | 无 |
| 3 | ObjectStorageContext | 0.5 天 | 无 |
| 4 | BillingContext | 1 天 | Phase 1 |
| 5a | StorageContext CRUD | 1 天 | Phase 1, 4 |
| 5b | StorageContext 其余 | 0.5 天 | 5a |
| 5c | OrchestratorContext chat | 1 天 | Phase 1–4 |
| 5d | OrchestratorContext dream | 0.5 天 | 5c |
| 5e | 清理 + transport-http | 1 天 | 5d |

Phase 2 和 3 可以并行（无依赖）。Phase 4 依赖 Phase 1。Phase 5 依赖 Phase 1–4。

---

## 7. 验证标准

每个 Phase 完成时：

- `cargo check -p app` 编译通过
- `cargo check -p transport-http` 编译通过
- `cargo test -p app --lib` 全部通过（当前 496 个）
- `cargo test -p app --test product_e2e product_e2e::smoke` 全部通过
- 现有方法签名不变（向后兼容），仅新增 `*_ctx()` 访问器
- 新代码不得引入 `#[allow(dead_code)]`（旧方法保留期间除外）
