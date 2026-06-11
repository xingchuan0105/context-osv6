# ADR: AppState 拆分 Phase 2–5

> 状态：Implemented + Optimized (2026-06-11)
> 日期：2026-06-11
> 前置：Phase 1（AnalyticsContext）已完成，commit 3316651
> 结果：AppState 从 22 字段减少到 7 字段，新增 5 个 Context 结构体，删除 4 个字段，合并 2 个字段

---

## 1. 背景

AppState 是 app crate 的核心结构体，原始状态有 22 个字段、15 个 impl 块（跨 11 个文件）、47 个文件引用它。它承担了认证、存储、LLM、RAG、编排、分析、计费、对象存储、限流的全部职责，是一个典型的 God Object。

Phase 1 已将 analytics 相关字段和方法提取为独立的 `AnalyticsContext`，旧方法保留为委托。Phase 2–5 继续按依赖关系渐进式拆分。

### 原始 AppState 字段（22 个）

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

### 实现

```rust
// crates/app/src/llm_context.rs
#[derive(Clone)]
pub struct LlmContext {
    llm_client: Option<LlmClient>,
    memory_llm_client: Option<LlmClient>,
}

impl LlmContext {
    pub fn new(llm_client: Option<LlmClient>, memory_llm_client: Option<LlmClient>) -> Self { ... }
    pub fn memory_llm_temperature(&self) -> Option<f32> { Some(0.2) }
    pub fn agent_llm_temperature(&self) -> Option<f32> { Some(0.2) }
    pub fn agent_client(&self) -> Option<&LlmClient> { self.llm_client.as_ref() }
    pub fn memory_client(&self) -> Option<&LlmClient> {
        self.memory_llm_client.as_ref().or(self.llm_client.as_ref())
    }
}
```

- AppState 字段：`llm_client` + `memory_llm_client` → `llm_ctx: LlmContext`
- `infer_profile_delta` 改用 `self.llm_ctx.memory_client()` / `self.llm_ctx.agent_client()`
- 向后兼容：`memory_llm_temperature()` / `agent_llm_temperature()` 委托到 `llm_ctx`

### 风险

极低。LLM client 的使用边界非常清晰，没有跨域耦合。

---

## 3. Phase 3: 提取 ObjectStorageContext

### 目标

将对象存储相关字段和方法从 AppState 移出。

### 实现

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
    pub fn new(...) -> Self { ... }
    pub fn object_store(&self) -> &Arc<ObjectStoreHandle> { ... }
    pub fn object_root_path(&self) -> &Path { ... }
    pub fn public_base_url(&self) -> &str { ... }
    pub fn signed_upload_url(&self, ...) -> Result<String, AppError> { ... }
    pub fn verify_upload_signature(&self, ...) -> Result<(), AppError> { ... }
    pub async fn resolve_citation_asset_url(&self, ...) -> Option<String> { ... }
}
```

- AppState 字段：`object_store` + `public_base_url` + `object_root` + `*_expire_sec` → `object_storage: ObjectStorageContext`
- `signed_upload_url` / `verify_upload_signature` 从 `chat_private.rs` 迁移到 `ObjectStorageContext`
- `resolve_citation_asset_url` 从 `asset_helpers.rs` 迁移到 `ObjectStorageContext`
- 向后兼容：AppState 保留委托方法

### 风险

低。对象存储边界清晰。`signed_upload_url` 的签名密钥从 `config_helpers` 导入。

---

## 4. Phase 4: 提取 BillingContext

### 目标

将计费/配额相关字段和方法从 AppState 移出。依赖 Phase 1（AnalyticsContext）。

### 实现

```rust
// crates/app/src/billing_context.rs
#[derive(Clone)]
pub struct BillingContext {
    quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    usage_limit_phase: String,
}

impl BillingContext {
    pub fn new(...) -> Self { ... }
    pub fn is_available(&self) -> bool { ... }
    pub fn usage_limit_phase(&self) -> &str { ... }
    pub fn quota_manager(&self) -> Option<&Arc<QuotaManager>> { ... }
    pub async fn get_user_usage_limit(&self, auth: &AuthContext) -> ... { ... }
    pub async fn check_user_quota(&self, auth: &AuthContext) -> ... { ... }
    pub(crate) async fn ensure_metric_quota(&self, auth: &AuthContext, ...) -> ... { ... }
    pub async fn record_llm_usage(&self, auth: &AuthContext, analytics: &AnalyticsContext, ...) { ... }
}
```

- AppState 字段：`quota_manager` + `usage_limit_phase` → `billing: BillingContext`
- `record_llm_usage` 内部同时写入 billing metering 和 analytics cost events
- 向后兼容：AppState 保留委托方法

### 风险

中。quota 检查嵌入在业务逻辑深处，需要仔细确认每个调用点的 auth 来源。

---

## 5. Phase 5: 分离 Storage 与 Orchestration

### 目标

将剩余的 AppState 字段拆为 `StorageContext`（数据持久化）和 `OrchestratorContext`（chat pipeline 编排）。

### 5.1 StorageContext

```rust
// crates/app/src/storage_context.rs
#[derive(Clone)]
pub struct StorageContext {
    pg: Option<Arc<PgAppRepository>>,
    inner: Arc<RwLock<MemoryState>>,
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    max_upload_file_size_bytes: u64,
    uses_memory_adapters: bool,
}

impl StorageContext {
    pub fn pg(&self) -> Option<Arc<PgAppRepository>> { ... }
    pub async fn pg_ready(&self) -> bool { ... }
    pub fn runtime_mode(&self) -> &'static str { ... }
    pub fn uses_memory_adapters(&self) -> bool { ... }
    pub fn max_upload_file_size_bytes(&self) -> u64 { ... }
    pub(crate) fn inner(&self) -> &Arc<RwLock<MemoryState>> { ... }
    pub(crate) fn api_keys(&self) -> &Arc<RwLock<BTreeMap<...>>> { ... }
    pub(crate) fn current_org_id(auth: &AuthContext) -> String { ... }
    pub(crate) fn current_user_id(auth: &AuthContext) -> String { ... }
}
```

- AppState 字段：`pg` + `inner` + `api_keys` + `max_upload_file_size_bytes` + `uses_memory_adapters` → `storage: StorageContext`
- 所有 CRUD 方法（notebooks, sessions, documents, preferences, notifications, api_keys）保留在 AppState 上，通过 `self.storage.pg()` / `self.storage.inner()` 访问
- 约 40 个文件的 `self.pg` / `self.inner` 引用批量替换为 `self.storage.pg()` / `self.storage.inner()`

### 5.2 OrchestratorContext

```rust
// crates/app/src/orchestrator_context.rs
#[derive(Clone)]
pub struct OrchestratorContext {
    agent_service: Option<Arc<UnifiedAgentService>>,
    chatmemory: Option<Arc<ChatMemory>>,
    guard_pipeline: Arc<GuardPipeline>,
}

impl OrchestratorContext {
    pub fn agent_service(&self) -> Option<Arc<UnifiedAgentService>> { ... }
    pub fn set_agent_service(&mut self, service: UnifiedAgentService) { ... }
    pub fn chatmemory(&self) -> Option<&Arc<ChatMemory>> { ... }
    pub fn guard_pipeline(&self) -> &Arc<GuardPipeline> { ... }
}
```

- AppState 字段：`agent_service` + `chatmemory` + `guard_pipeline` → `orchestrator: OrchestratorContext`
- chat pipeline 方法保留在 AppState 上，通过 `self.orchestrator.*()` 访问

### 5.3 transport-http 适配

采用选择 A：AppState 保留为 handler 入口，handler 内部通过 `state.storage()`、`state.orchestrator()` 等获取子上下文。不改 handler 签名。

---

## 6. 实施终态

### AppState 字段（22 → 7）

Phase 2–5 完成后 + 优化清理：

```rust
pub struct AppState {
    pub(crate) auth: AuthContext,                          // per-request
    pub(crate) storage: StorageContext,                    // per-app (含 object_store)
    pub(crate) llm_ctx: LlmContext,                       // per-app
    pub(crate) orchestrator: OrchestratorContext,          // per-app (含 rag_runtime)
    pub(crate) analytics: AnalyticsServiceCtx,            // per-app
    pub(crate) billing: BillingContext,                    // per-app
    pub(crate) redis_url: String,                         // middleware config
}
```

### 优化清理记录

| 操作 | 字段 | 原因 |
|------|------|------|
| 删除 | `key_vault` | 整个代码库零调用者，死代码 |
| 删除 | `search_provider` | 仅 1 个 debug 读取点，已移除 |
| 删除 | `search_mode` | 仅 1 个 debug 读取点，已移除 |
| 移入 OrchestratorContext | `rag_runtime` | rag 与 agent 编排同属一个关注点 |
| 合并入 StorageContext | `object_storage` | 对象存储与数据持久化同属一个关注点 |
| 包装 | `analytics` | 用 AnalyticsServiceCtx 包装 Option<Arc<>>，提供 into_context() |

### Context 结构体汇总

| Context | 文件 | 字段 | 方法数 |
|---------|------|------|--------|
| `AnalyticsServiceCtx` | `analytics_context.rs` | analytics service | 3 (is_available/service/into_context) |
| `AnalyticsContext` | `analytics_context.rs` | analytics service, actor_id, request_id | 5 (record_*) |
| `LlmContext` | `llm_context.rs` | llm_client, memory_llm_client | 5 (client/temperature) |
| `BillingContext` | `billing_context.rs` | quota_manager, usage_limit_phase | 6 (quota/usage) |
| `StorageContext` | `storage_context.rs` | pg, inner, api_keys, object_store, config | 15 (pg/object/accessors) |
| `OrchestratorContext` | `orchestrator_context.rs` | agent_service, chatmemory, guard_pipeline, rag_runtime | 5 (accessors) |

### 向后兼容

所有旧的 AppState 方法保留为委托方法，外部调用方（transport-http、bins、tests）无需修改。

### 验证结果

- `cargo check -p app` ✅ 零警告
- `cargo check -p transport-http` ✅
- `cargo test -p app --lib` — 496/496 通过 ✅

---

## 7. 进一步优化空间

对 AppState 剩余 7 个字段的分析：

| 字段 | 访问次数 | 状态 | 说明 |
|------|---------|------|------|
| `key_vault` | 0 个调用者 | ✅ 已删除 | 死代码 |
| `search_provider` | 1 个读取点 | ✅ 已删除 | debug metadata 不值得一个字段 |
| `search_mode` | 1 个读取点 | ✅ 已删除 | 同上 |
| `rag_runtime` | 2 个消费者 | ✅ 已移入 | OrchestratorContext |
| `object_storage` | 8 个调用点 | ✅ 已合并 | 合入 StorageContext |
| `analytics` | 39 个调用点 | ✅ 已包装 | AnalyticsServiceCtx 包装 Option<Arc<>> |
| `redis_url` | 2 个读取点 | 保留 | 仅被 HTTP rate limiter 使用，移入中间件层需改 47 处 build_router 调用 |

### 结论

7 字段是当前架构的合理终态。`redis_url` 的进一步迁移收益不足以证明其改动成本（需改 47 处 `build_router` 调用）。
