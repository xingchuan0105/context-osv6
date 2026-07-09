# LLM Usage Exit-Point Metering — Design Spec (v2)

**Date**: 2026-07-05
**Status**: Draft (revised after code review)
**Author**: AI + chuan

## 1. Problem Statement

当前 usage 计量存在三个核心问题：

1. **计量不完整** — chat pipeline 仅记 `llm_input_tokens` / `llm_output_tokens` 到 `usage_events`（估算值）和 `llm_usage_events`（实际值）。worker 侧的 triplet extraction、VLM figure summary、doc summary 的 LLM 调用虽有部分写入 `llm_usage_events`（`document_pipeline.rs:555-567`），但未覆盖全部路径（如 VLM `complete()` 没有）。embedding 调用的 token 不记 `llm_usage_events`（仅 worker processor 估算后写 `usage_events`）。reranker 完全不计量。
2. **数据源不一致** — 月度限额查 `usage_events`（估算 token），滚动窗口查 `llm_usage_events`（实际 token）。两表口径不同，同一用户的同一次 chat 请求在两表里的数值可能相差数倍。
3. **enforcement 半开半关** — `enforcement_phase` 环境变量控制 `check_user_quota()` 的阻断，但 `QuotaManager::check_quota()`（`ensure_metric_quota` 内部）的滚动检查不受控，行为不一致。

**目标**：在 LLM API 出口处统一、全量记录 token 消耗，以租户（org_id + user_id）为单位，实现真正的 5h/7d 硬限额阻断，消除两表口径差异。

## 2. Architecture Overview

### 核心思路

在 `LlmClient` 和 `EmbeddingClient` 内部，**LLM API 调用返回后、结果交给调用方之前**，插入计量钩子。计量逻辑通过 `UsageObserver` trait 注入，`llm` crate 只定义 trait 和数据结构，实现在 `app-billing` crate。

```
┌──────────────────────────────────────────────────┐
│ 调用方 (agent loop / worker / summary / RAG)      │
│   ↓ clone LlmClient/EmbeddingClient + inject      │
├──────────────────────────────────────────────────┤
│ LlmClient / EmbeddingClient  (llm crate)          │
│   ↓ post_chat_completions() → LlmUsage            │
│   ↓ observer.record_chat(tenant, usage).await     │
│   ↓ 返回 LlmResponse                             │
├──────────────────────────────────────────────────┤
│ UsageObserver impl  (app-billing crate)            │
│   ↓ 写 llm_usage_events 表                        │
│   ↓ 折算 usage_units（读 llm_model_weights）       │
├──────────────────────────────────────────────────┤
│ QuotaManager::check_quota()                       │
│   ↓ 读 llm_usage_events → 5h / 7d / 月度限额      │
│   ↓ 阻断 or 放行                                  │
└──────────────────────────────────────────────────┘
```

### UsageObserver trait（`crates/llm/src/usage_observer.rs`）

零外部依赖（仅 `uuid::Uuid` + `async_trait`）：

```rust
#[async_trait]
pub trait UsageObserver: Send + Sync {
    /// Record a chat-completion LLM call at the API exit point.
    /// Called with the actual token counts returned by the provider.
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord);

    /// Record an embedding LLM call at the API exit point.
    /// `actual_tokens` is Some only for DashScope multimodal embedding responses.
    async fn record_embedding(&self, tenant: &TenantContext, record: &EmbeddingUsageRecord);
}

pub struct TenantContext {
    pub org_id: Uuid,
    pub user_id: Uuid,        // Uuid::nil() if unknown (system tasks)
}

pub struct ChatUsageRecord {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub provider: String,
    pub model: String,
    pub feature: String,       // "agent_loop" / "summary" / "triplet" / "vlm" / ...
    pub stage: String,         // caller-defined: "chat"/"rag"/"search"/"worker_summary"/...
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

pub struct EmbeddingUsageRecord {
    pub estimated_tokens: u32,
    pub actual_tokens: Option<u32>,   // DashScope multimodal only
    pub provider: String,
    pub model: String,
    pub feature: String,       // "document_embedding" / "query_embedding" / ...
}
```

### Observer 失败策略

Observer 调用 **fail-open**：写 DB 失败只记录 warn log，不阻塞 LLM 响应返回给用户。

`LlmClient::record_completion_success()` 当前是同步函数。改为 `async`，内部 await observer：

```rust
async fn record_completion_success(&self, call: &CompletionCall, model: &str, usage: &ApiUsageRaw, ...) {
    // ... existing prometheus telemetry (unchanged) ...
    self.rate_limit.record_usage(call.pre_deducted, usage.total_tokens() as usize);

    if let Some((obs, tenant)) = &self.observer {
        let record = ChatUsageRecord { ... };
        if let Err(e) = obs.record_chat(tenant, &record).await {
            tracing::warn!(error = %e, "usage observer record_chat failed; continuing");
        }
    }
}
```

调用方 `complete_non_stream()` 和 `complete_stream()` 中，`record_completion_success()` 改为 `.await`。不影响现有逻辑。

对于 in-flight observer 写入丢失：observer 写 DB 是同步 await 的——如果进程在 write 返回前崩溃，该次调用不会被记。这是可接受的行为（与 LLM provider 返回了 usage 但响应未送达调用方的场景对等）。不需要 channel / spawn 后台写。

### 注入方式

```rust
// LlmClient
pub fn with_observer(mut self, observer: Arc<dyn UsageObserver>, tenant: TenantContext) -> Self {
    self.observer = Some((observer, tenant));
    self
}

// EmbeddingClient
pub fn with_observer(mut self, observer: Arc<dyn UsageObserver>, tenant: TenantContext) -> Self {
    self.observer = Some((observer, tenant));
    self
}
```

不传 observer 时行为不变（测试、eval 不受影响）。

### 钩子调用点

- **`LlmClient`**：`record_completion_success()`（`client/mod.rs:194-218`）末尾，已有 `call.provider`、`resp.model`、`ApiUsageRaw` 全部信息。`feature` 取自 `self.feature`（已有字段），`stage` 各调用方预留为当前未用字段（后续归因需求通过 request_id 关联）。
- **`EmbeddingClient`**：
  - `embed_openai_compatible_text()` 返回前（`embedding.rs:476`），`estimated_tokens = estimate_tokens_for_texts(batch)`，`actual_tokens = None`
  - `embed_multimodal_fused()` 返回前（L407），`estimated_tokens = input.estimate_tokens()`，`actual_tokens = resp.usage.total_tokens`

### 缓存命中免费

- `LlmClient` 的 `CompletionCache`（triplet extraction 使用）：缓存命中不发起 HTTP 请求，不走 `record_completion_success()`，**不计入用量**。这是出口计量的正确性质。
- `EmbeddingClient` 的 Redis 缓存：缓存命中返回已有向量，不走 API 调用，**不计入用量**。

## 3. TenantContext Propagation

`LlmClient` 和 `EmbeddingClient` 均已有 `#[derive(Clone)]`（`client/mod.rs:22`、`embedding.rs:154`），clone 成本低（内部 `reqwest::Client` 是 `Arc`-based）。

### 传播路径

```
bootstrap
  LlmClient::new(config)           ← 无 observer
  EmbeddingClient::new(config)     ← 无 observer
       │ 存储在 AppState / PgTaskProcessor / RagConfig
       │
请求/任务到来（有 org_id, user_id）
       │
       ├─ Agent 路径 (UnifiedAgent::run)
       │   unified/mod.rs L142-146 (chat), L207-208 (rag), L254-258 (search)
       │   → client.clone().with_observer(obs, tenant)
       │   → Arc::new(client) → ReActLoop
       │
       ├─ Worker 路径 (PgTaskProcessor::process)
       │   processor.rs L114: task_context(task) → AuthContext → TenantContext
       │   → embedding_client.clone().with_observer(obs, tenant)
       │   → triplet_llm.clone() → 需要通过 Arc 解引用后 clone + inject
       │   → ingestion_llm.clone() → 同上
       │   → summary_generator 内部 LlmClient ← agent_llm client 需注入
       │
       ├─ Memory/Dream 路径 (ChatContext)
       │   ChatContext 已有 AuthContext（org_id + user_id）
       │   → memory_llm_client 在首次使用时 clone + inject
       │
       └─ RAG Query Embedding 路径 (RagRuntime)
           rag-core/src/runtime/config.rs:11 embedding_client: Arc<EmbeddingClient>
           → RagRuntime 查询入口传入 TenantContext
           → 在 retrieval.rs:110、graph.rs:52、graph_augment.rs:76 
             使用处 clone + inject observer
```

### Worker 侧 user_id 缺失处理

`task_context()` 中 `requested_by` 为 `Option<Uuid>`（系统任务可能无 user_id）。
`TenantContext.user_id` 填 `Uuid::nil()`，observer 写入 `llm_usage_events.user_id = Uuid::nil()`。月度限额按 `user_id` 聚合时，nil 归属系统租户，不计入个人限额。

### Ingestion 预检的 user_id 同样处理

预检时若 `task.requested_by` 为 None，user_id 用 `Uuid::nil()`，`QuotaManager::check_quota()` 对其行为等同于 enterprise（所有限额为 0 = unlimited）。

## 4. Data Model Unification

### 现状

| 表 | 写入者 | 数据质量 | 用途 |
|---|---|---|---|
| `usage_events` | `ChatContext::record_usage()`, worker processor | 估算 token | 月度容量配额 |
| `llm_usage_events` | `record_usage_for_execution()` (chat), worker `document_pipeline.rs:555` (doc summary) | 实际 + 估算 | 滚动窗口检查 |

### 方案：以 `llm_usage_events` 为唯一真实数据源

#### 表增强（Migration 0050）

```sql
-- 0050_llm_usage_exit_metering.up.sql

-- 1. Add usage_kind column to distinguish call types
ALTER TABLE llm_usage_events
  ADD COLUMN IF NOT EXISTS usage_kind TEXT NOT NULL DEFAULT 'chat';
  -- 'chat' | 'embedding_text' | 'embedding_multimodal'

-- 2. Index for monthly queries by kind
CREATE INDEX IF NOT EXISTS idx_llm_usage_user_kind_time
  ON llm_usage_events(user_id, usage_kind, created_at DESC);

-- 3. Add feature column if not already present (migration 0018 already has it)
--    feature is NOT NULL in 0018, so backfill not needed for new rows.
```

存量行的 `usage_kind` 自动填 `'chat'`（DEFAULT），无需显式 UPDATE。

#### Embedding 的 token 落列策略

`llm_usage_events` 已有 `total_tokens` 列。embedding 的 token 写入 `total_tokens`：

- 文本 embedding：`total_tokens = estimated_tokens`，`usage_source = 'estimated'`
- 多模态 embedding：`total_tokens = actual_tokens`，`usage_source = 'actual'`

`prompt_tokens` / `completion_tokens` 填 0（embedding 调用无此区分）。

#### 月度限额查询切换

`QuotaManager::check_quota()` → `check_monthly_quota()` → `current_metric_usage()` 当前读 `usage_events` 表。改为读 `llm_usage_events`：

- `llm_input_tokens` 月用量：`SUM(prompt_tokens) WHERE usage_kind='chat' AND user_id=$1 AND created_at >= month_start`
- `llm_output_tokens` 月用量：`SUM(completion_tokens) WHERE usage_kind='chat' AND user_id=$1 AND created_at >= month_start`
- `embedding_tokens` 月用量：`SUM(total_tokens) WHERE usage_kind LIKE 'embedding%' AND user_id=$1 AND created_at >= month_start`

直接改 `crates/app-bootstrap/src/adapters/billing_sql/core_usage.rs::current_metric_usage()` 的实现，其它调用方不变。

#### `usage_events` 表处理

- Worker 的 `pages_processed` 写入保留（非 LLM 指标，不迁入 `llm_usage_events`）
- Worker 的 `triplet_extraction_tokens` 写入保留
- Worker 的 `embedding_tokens` 写入移除（observer 覆盖）
- Chat 的 `llm_input_tokens` / `llm_output_tokens` 写入移除（observer 覆盖）
- 表结构和数据保留（迁移兼容），不再作为月度限额的数据源

#### `record_usage_event()` 调用点梳理及处理

| 位置 | metric_type | 处理 |
|---|---|---|
| `chat/service_postprocess.rs:229` | `llm_input_tokens` | 移除 |
| `chat/service_postprocess.rs:236` | `llm_output_tokens` | 移除 |
| `worker/processor.rs:410` | `pages_processed` | **保留**（非 LLM） |
| `worker/processor.rs:419` | `embedding_tokens` | **移除**（observer 覆盖） |
| `worker/document_pipeline.rs:670` | `triplet_extraction_tokens` | **保留**（非 LLM 直接指标） |

## 5. Enforcement Integration

### 现状（三条链路，行为不一致）

```
chat/service.rs execute_chat_preflight()
  ├─ ensure_metric_quota("llm_input_tokens")  ← QuotaManager → 滚动+月度，始终生效
  ├─ ensure_metric_quota("llm_output_tokens") ← 同上
  └─ check_user_quota()                       ← 仅滚动，受 enforcement_phase 控制
```

### 方案：简化为单一路径（QuotaManager）

```
chat/service.rs execute_chat_preflight()
  └─ QuotaManager::check_quota(org_id, user_id, "llm_input_tokens", estimated)
       ├─ 读 llm_usage_events → 检查 5h 滚动 → 超限 → 429 usage_limit_exceeded
       ├─ 读 llm_usage_events → 检查 7d 滚动 → 超限 → 429 usage_limit_exceeded
       └─ 读 llm_usage_events → 检查月度硬限制 → 超限 → 429 quota_exceeded
```

### 具体变更

1. **删除 `check_user_quota()` 独立调用和 `enforcement_phase` 逻辑**
   - `chat/service.rs:58-106` 整个 phase 判断 + 两段 `blocked_5h` / `blocked_7d` 检查移除
   - `UsageLimitConfig.enforcement_phase` 标记 `#[deprecated]`，不再从环境变量读取
   - `BillingContext.usage_limit_phase` 字段保留（前端可能展示用量，但不再用于阻断开关）

2. **`ensure_metric_quota()` 保留并增强**
   - `chat/service.rs:54-56` 的两次 `ensure_metric_quota` 调用合并为一次 `QuotaManager::check_quota("llm_input_tokens", estimated)`（llm_output_tokens 无需独立预检——输出 token 在输入检查中已经代表）
   - 或者保留两次调用但将 `llm_output_tokens` 预估值从硬编码 1024 改为合理的 output token cap

3. **阻断错误码**
   - 滚动超限 → `usage_limit_exceeded` + `Retry-After` header（已有）
   - 月度超限 → `quota_exceeded` + `Retry-After` header（已有）

4. **Worker ingestion 预检**
   - 在 `PgTaskProcessor::process()` 处理 pipeline 前，调用 `QuotaManager::check_quota(org_id, user_id, "embedding_tokens", estimated)`
   - 超限 → 任务标记 `failed`，返回 `quota_exceeded`
   - `user_id` 为 `Uuid::nil()` 时（`requested_by` 缺失），跳过预检（系统任务不受限）

## 6. 原子切换（避免 double-counting）

### 为什么不能并行双写

现有 `record_usage_for_execution()`（`service_postprocess.rs:269`）和 worker `document_pipeline.rs:555-567` 已经通过 `rolling_service().record_usage()` 写入 `llm_usage_events`。如果 observer 也写同一张表，`sum_usage_units_since()` 直接翻倍——用户在约一半限额时就会被 429 阻断。

### 切换方案：同一部署原子替换

1. **部署 observer 代码**：
   - Observer trait + impl 就位
   - `LlmClient` / `EmbeddingClient` 内部钩子就位
   - 但**不注入 observer**（没有 `with_observer()` 调用）→ 旧路径不变

2. **同一 PR 中，注入 observer 并移除旧写入**：
   - 所有 inject 点添加 `with_observer()` 调用
   - **同时**移除 `service_postprocess.rs:223-289`（`record_usage_for_execution`）中对 `llm_usage_events` 的写入
   - **同时**移除 `document_pipeline.rs:545-567` 中对 `llm_usage_events` 的 worker doc summary 写入
   - **同时**移除 `worker/processor.rs:419` 的 `embedding_tokens` 写入
   - 旧路径中对 `usage_events` 的写入保留（`pages_processed`、`triplet_extraction_tokens` 不变）

3. **切月度数据源**：`current_metric_usage()` 从 `usage_events` 切到 `llm_usage_events`

4. **观察验证**：部署后对比 Prometheus 的 `llm_usage_events` 写入量，确认与旧路径数量级一致

## 7. Scope Coverage

### 将被计量的全部 LLM 调用路径

| 路径 | Client | inject 点 | feature |
|---|---|---|---|
| Agent loop chat | `LlmClient` (agent_llm) | `unified/mod.rs:142` | `"agent_loop"` |
| Agent loop chat (dedicated) | `LlmClient` (chat_llm) | `unified/mod.rs:143` | `"agent_loop"` |
| Agent loop rag | `LlmClient` (agent_llm) | `unified/mod.rs:207` | `"agent_loop"` |
| Agent loop search | `LlmClient` (search_llm/agent_llm) | `unified/mod.rs:254` | `"agent_loop"` |
| Memory dream delta | `LlmClient` (memory_llm) | ChatContext 构造 | `"memory"` |
| RAG query embedding | `EmbeddingClient` | RagRuntime retrieval entry | `"query_embedding"` |
| Graph entity embedding | `EmbeddingClient` | RagRuntime graph entry | `"graph_embedding"` |
| Worker triplet extraction | `LlmClient` (triplet_llm) | `processor.rs:94` | `"triplet"` |
| Worker VLM figure summary | `LlmClient` (ingestion_llm) | `pdf/b_class.rs:204` | `"vlm_summary"` |
| Worker doc summary (SummaryGenerator) | `LlmClient` (agent_llm) | SummaryGenerator 内部 | `"summary"` |
| Worker section index | `LlmClient` (agent_llm) | SectionIndexGenerator 内部 | `"section_index"` |
| Worker document embedding | `EmbeddingClient` | `processor.rs:82-83` | `"document_embedding"` |

### Out of Scope

| 项目 | 说明 |
|---|---|
| `RerankerClient` 计量 | API 不返回 token usage，后续 provider 支持后补 |
| `BillableFeature::Planner` | 枚举已定义但 `RetrievalPlanner` 内部无独立 LLM 调用（走 agent loop） |
| `BillableFeature::GraphExtraction` | 枚举已定义但无实现 |
| `usage_events` 表删除 | 仅停止部分写入，表保留确保迁移兼容 |

## 8. Migration

### Migration 0050 (`0050_llm_usage_exit_metering.up.sql`)

```sql
ALTER TABLE llm_usage_events
  ADD COLUMN IF NOT EXISTS usage_kind TEXT NOT NULL DEFAULT 'chat';

CREATE INDEX IF NOT EXISTS idx_llm_usage_user_kind_time
  ON llm_usage_events(user_id, usage_kind, created_at DESC);
```

### Code Changes

| Crate | File | Change |
|---|---|---|
| `llm` | `src/usage_observer.rs` | **New**: `UsageObserver` trait, `TenantContext`, record types |
| `llm` | `src/client/mod.rs` | Add `observer` field, `with_observer()`, call in `record_completion_success()` (make async) |
| `llm` | `src/embedding.rs` | Add `observer` field, `with_observer()`, call in `embed_openai_compatible_text()` and `embed_multimodal_fused()` |
| `llm` | `src/lib.rs` | Re-export `UsageObserver`, `TenantContext`, record types |
| `app-billing` | `src/usage_observer_impl.rs` | **New**: `PgUsageObserver` impl, writes to `llm_usage_events` via `PgUsageLimitStoreAdapter` |
| `app-chat` | `src/agents/unified/mod.rs` | L142-146 (chat), L207-208 (rag), L254-258 (search): `.with_observer(obs, tenant)` |
| `app-chat` | `src/chat/service.rs` | Remove `check_user_quota()` phase-gated block (L58-106), simplify `ensure_metric_quota` |
| `app-chat` | `src/chat/service_postprocess.rs` | Remove `record_usage_for_execution()` calls writing to `llm_usage_events` (L269-289) |
| `app-chat` | `src/chat_private/quota.rs` | Mark `record_usage()` to `usage_events` deprecated |
| `app-chat` | `src/chat_private/mod.rs` | Mark `record_llm_usage_if_available()` deprecated (observer covers it) |
| `app-bootstrap` | `src/lib.rs` | Create `PgUsageObserver` at AppState init, inject at all construction points |
| `app-bootstrap` | `src/adapters/billing_sql/core_usage.rs` | `current_metric_usage()` switch from `usage_events` to `llm_usage_events` |
| `app-core` | `src/config.rs` | Mark `UsageLimitConfig.enforcement_phase` deprecated |
| `rag-core` | `src/runtime/retrieval.rs` | Accept `TenantContext`, inject observer into `embedding_client` clone |
| `rag-core` | `src/runtime/tools/graph.rs` | Accept `TenantContext`, inject observer into `embedding_client` clone |
| `rag-core` | `src/runtime/tools/graph_augment.rs` | Accept `TenantContext`, inject observer into `embedding_client` clone |
| `bins/worker` | `src/pipeline/processor.rs` | Inject observer into `embedding_client`, `triplet_llm`, `ingestion_llm`; remove L419 `embedding_tokens` write |
| `bins/worker` | `src/pipeline/document_pipeline.rs` | Remove L545-567 `record_usage()` to `llm_usage_events` (observer covers it via `LlmClient`) |
| `bins/worker` | `src/runtime_support.rs` | Build `PgUsageObserver` and inject into worker clients |

## 9. Testing Strategy

### Unit Tests
- `UsageObserver` mock: verify correct record fields passed for chat and embedding
- `LlmClient::with_observer()`: observer not called when None; called when Some
- `EmbeddingClient::with_observer()`: text path uses estimated_tokens; multimodal uses actual_tokens

### Integration Tests
- End-to-end chat: verify `llm_usage_events` row with correct `org_id`, `user_id`, `usage_kind='chat'`, feature
- End-to-end ingestion: verify embedding usage recorded with `usage_kind='embedding_text'`
- Quota boundary: exhaust 5h rolling window → HTTP 429 `usage_limit_exceeded`

### Backward Compatibility
- No observer injection → behavior unchanged
- Tests using `LlmClient::new(config)` / `EmbeddingClient::new(config)` unaffected

## 10. Rollout

1. Deploy migration 0050 (`usage_kind` column + index, backward compatible)
2. Deploy code with observer + atomic switch (single PR: inject observer AND remove old writes simultaneously)
3. Verify Prometheus: `llm_usage_events` insert rate matches pre-deployment level (not doubled)
4. Switch `current_metric_usage()` to `llm_usage_events` in a follow-up PR
5. Remove `enforcement_phase` config, drop deprecated code in a final cleanup PR
