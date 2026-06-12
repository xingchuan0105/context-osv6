# AppState Decomposition Phase 2–4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `LlmContext`, `ObjectStorageContext`, and `BillingContext` from AppState, reducing its field count from 22 to ~13.

**Architecture:** Each context is a `#[derive(Clone)]` struct owning its fields, with methods that previously lived on AppState. AppState retains a `*_ctx()` accessor (like Phase 1's `analytics_ctx()`). Old methods on AppState are preserved as legacy delegates for backward compatibility. Phases 2 and 3 are independent; Phase 4 depends on Phase 1 (done).

**Tech Stack:** Rust, `avrag-llm`, `avrag-storage-pg`, `avrag-billing`, `analytics`

---

## File Map

| Action | File | Phase |
|--------|------|-------|
| Create | `crates/app/src/llm_context.rs` | 2 |
| Create | `crates/app/src/object_storage_context.rs` | 3 |
| Create | `crates/app/src/billing_context.rs` | 4 |
| Modify | `crates/app/src/lib.rs` | 2,3,4 |
| Modify | `crates/app/src/lib_impl/state_types.rs` | 2,3,4 |
| Modify | `crates/app/src/lib_impl/state_methods.rs` | 2,3,4 |
| Modify | `crates/app/src/lib_impl/chat_private.rs` | 2,4 |
| Modify | `crates/app/src/lib_impl/chat_streaming.rs` | 2 |
| Modify | `crates/app/src/lib_impl/asset_helpers.rs` | 3 |
| Modify | `crates/app/src/lib_impl/documents.rs` | 3 |
| Modify | `crates/app/src/lib_impl/chat/service_postprocess.rs` | 4 |
| Modify | `crates/transport-http/src/lib_impl/auth_secondary.rs` | 4 |
| Modify | `crates/transport-http/src/lib_impl/infra_handlers.rs` | 3 |
| Modify | `crates/app/src/lib_impl/tests.rs` | 2,3,4 |

---

## Task 1: Phase 2 — Create LlmContext

**Covers:** ADR §2 (Phase 2: 提取 LlmContext)

**Files:**
- Create: `crates/app/src/llm_context.rs`
- Modify: `crates/app/src/lib.rs:15` (add module)
- Modify: `crates/app/src/lib_impl/state_types.rs:21-22` (replace fields)

- [ ] **Step 1: Create `llm_context.rs`**

```rust
// crates/app/src/llm_context.rs
use avrag_llm::LlmClient;

#[derive(Clone)]
pub struct LlmContext {
    llm_client: Option<LlmClient>,
    memory_llm_client: Option<LlmClient>,
}

impl LlmContext {
    pub fn new(
        llm_client: Option<LlmClient>,
        memory_llm_client: Option<LlmClient>,
    ) -> Self {
        Self {
            llm_client,
            memory_llm_client,
        }
    }

    pub fn memory_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn agent_llm_temperature(&self) -> Option<f32> {
        Some(0.2)
    }

    pub fn agent_client(&self) -> Option<&LlmClient> {
        self.llm_client.as_ref()
    }

    pub fn memory_client(&self) -> Option<&LlmClient> {
        self.memory_llm_client.as_ref().or(self.llm_client.as_ref())
    }
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `pub mod llm_context;` after line 15 (`pub mod analytics_context;`).

- [ ] **Step 3: Update `state_types.rs` — replace fields**

Replace lines 21-22:
```rust
    pub(crate) llm_client: Option<LlmClient>,
    pub(crate) memory_llm_client: Option<LlmClient>,
```
With:
```rust
    pub(crate) llm_ctx: crate::llm_context::LlmContext,
```

Remove the `use avrag_llm::LlmClient;` import at line 5 if no longer needed in this file.

- [ ] **Step 4: Update `state_methods.rs` — constructors**

In `AppState::new()` (line 35), replace the field construction. Change:
```rust
        let llm_client = make_llm_client(&config.agent_llm);
        let memory_llm_client = make_llm_client(&config.memory_llm);
```
to:
```rust
        let llm_ctx = crate::llm_context::LlmContext::new(
            make_llm_client(&config.agent_llm),
            make_llm_client(&config.memory_llm),
        );
```

In the `Self { ... }` block of `new()`, replace `llm_client,` and `memory_llm_client,` with `llm_ctx,`.

In `AppState::bootstrap()` (line 92), same change — construct `llm_ctx` and use it in the `Self { ... }` block.

Note: `llm_client.clone()` is used at line 53 (`build_unified_agent_service`) and line 201. Change to `llm_ctx.agent_client().cloned()`.

- [ ] **Step 5: Add accessor and temperature delegate methods**

In `state_methods.rs`, add to the first `impl AppState` block:
```rust
    pub fn llm_ctx(&self) -> &crate::llm_context::LlmContext {
        &self.llm_ctx
    }
```

Keep `memory_llm_temperature()` and `agent_llm_temperature()` as delegates:
```rust
    pub fn memory_llm_temperature(&self) -> Option<f32> {
        self.llm_ctx.memory_llm_temperature()
    }

    pub fn agent_llm_temperature(&self) -> Option<f32> {
        self.llm_ctx.agent_llm_temperature()
    }
```

- [ ] **Step 6: Run `cargo check -p app`**

Verify compilation passes. Fix any remaining references to `self.llm_client` or `self.memory_llm_client`.

---

## Task 2: Phase 2 — Migrate `infer_profile_delta` Callers

**Covers:** ADR §2 (infer_profile_delta migration)

**Files:**
- Modify: `crates/app/src/lib_impl/chat_private.rs:122-177`

- [ ] **Step 1: Update `infer_profile_delta` to use `LlmContext`**

Change the method signature and body in `chat_private.rs` (line 122):
```rust
    pub(crate) async fn infer_profile_delta(
        &self,
        recent_turns: &str,
        existing_profile: &serde_json::Value,
    ) -> serde_json::Value {
```

Replace the LLM client iteration (lines 138-141):
```rust
        for (llm, temperature) in [
            (&self.memory_llm_client, self.memory_llm_temperature()),
            (&self.llm_client, self.agent_llm_temperature()),
        ] {
```
With:
```rust
        for (llm, temperature) in [
            (self.llm_ctx.memory_client(), self.llm_ctx.memory_llm_temperature()),
            (self.llm_ctx.agent_client(), self.llm_ctx.agent_llm_temperature()),
        ] {
```

Note: `memory_client()` returns `Option<&LlmClient>` (not `&Option<LlmClient>`), so the `if let Some(client) = llm` pattern still works.

- [ ] **Step 2: Run `cargo check -p app`**

Verify compilation passes.

- [ ] **Step 3: Run tests**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app --lib
```

---

## Task 3: Phase 3 — Create ObjectStorageContext

**Covers:** ADR §3 (Phase 3: 提取 ObjectStorageContext)

**Files:**
- Create: `crates/app/src/object_storage_context.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/lib_impl/state_types.rs:28,31-32,37-38`
- Modify: `crates/app/src/lib_impl/state_methods.rs`
- Modify: `crates/app/src/lib_impl/asset_helpers.rs:48-67`
- Modify: `crates/app/src/lib_impl/chat_private.rs:374-425`

- [ ] **Step 1: Create `object_storage_context.rs`**

```rust
// crates/app/src/object_storage_context.rs
use std::path::Path;
use std::sync::Arc;
use avrag_storage_pg::ObjectStoreHandle;
use common::AppError;

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
    ) -> Self {
        Self {
            object_store,
            public_base_url,
            object_root,
            upload_expire_sec,
            download_expire_sec,
        }
    }

    pub fn object_store(&self) -> &Arc<ObjectStoreHandle> {
        &self.object_store
    }

    pub fn object_root_path(&self) -> &Path {
        Path::new(&self.object_root)
    }

    pub fn public_base_url(&self) -> &str {
        &self.public_base_url
    }

    pub fn download_expire_sec(&self) -> u64 {
        self.download_expire_sec
    }

    pub fn upload_expire_sec(&self) -> u64 {
        self.upload_expire_sec
    }

    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        use crate::lib_impl::chat_private::{sign_upload_payload, upload_signing_secret};

        let expires = expires_at_unix.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_secs())
                .unwrap_or_default()
                + self.upload_expire_sec
        });
        let signature =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        Ok(format!(
            "{}/uploads/{}?expires={}&signature={}",
            self.public_base_url, document_id, expires, signature
        ))
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        use crate::lib_impl::chat_private::{sign_upload_payload, upload_signing_secret};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        if expires < now {
            return Err(AppError::validation(
                "upload_url_expired",
                "upload url expired",
            ));
        }
        let expected =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        if expected != signature {
            return Err(AppError::validation(
                "invalid_upload_signature",
                "invalid upload signature",
            ));
        }
        Ok(())
    }

    pub async fn resolve_citation_asset_url(
        &self,
        asset: &avrag_storage_pg::DocumentAssetRow,
    ) -> Option<String> {
        use crate::lib_impl::asset_helpers::is_remote_asset_reference;

        let storage_path = asset.storage_path.as_deref()?;
        if is_remote_asset_reference(storage_path) {
            return Some(storage_path.to_string());
        }

        match self
            .object_store
            .presigned_get_url(storage_path, self.download_expire_sec)
            .await
        {
            Ok(url) if !url.starts_with("file://") => Some(url),
            _ => Some(format!("/api/v1/chat/citations/assets/{}", asset.asset_id)),
        }
    }
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `pub mod object_storage_context;` after `pub mod llm_context;`.

- [ ] **Step 3: Update `state_types.rs` — replace fields**

Replace lines 28, 31-32, 37-38:
```rust
    pub(crate) object_store: Arc<ObjectStoreHandle>,
    ...
    pub(crate) public_base_url: String,
    pub(crate) object_root: String,
    ...
    pub(crate) object_storage_upload_expire_sec: u64,
    pub(crate) object_storage_download_expire_sec: u64,
```
With:
```rust
    pub(crate) object_storage: crate::object_storage_context::ObjectStorageContext,
```

Remove unused imports (`ObjectStoreHandle` if no longer needed).

- [ ] **Step 4: Update `state_methods.rs` — constructors**

In both `new()` and `bootstrap()`, construct `ObjectStorageContext`:
```rust
        let object_storage = crate::object_storage_context::ObjectStorageContext::new(
            object_store,
            config.public_base_url.clone(),
            config.object_root.clone(),
            config.object_storage.upload_url_expire_sec,
            config.object_storage.download_url_expire_sec,
        );
```

In `new()`, the local `object_store` variable is constructed at line 37. Use it to build `object_storage` and then use `object_storage` in the `Self { ... }` block.

Replace field assignments in `Self { ... }`:
- Remove `object_store,`
- Remove `public_base_url: config.public_base_url,`
- Remove `object_root: config.object_root,`
- Remove `object_storage_upload_expire_sec: ...`
- Remove `object_storage_download_expire_sec: ...`
- Add `object_storage,`

Add accessor:
```rust
    pub fn object_storage(&self) -> &crate::object_storage_context::ObjectStorageContext {
        &self.object_storage
    }
```

- [ ] **Step 5: Update `asset_helpers.rs`**

Remove the `impl AppState` block (lines 48-67) that defines `resolve_citation_asset_url`. This method is now on `ObjectStorageContext`.

Make `is_remote_asset_reference` (line 77) `pub(crate)` so `ObjectStorageContext` can use it.

- [ ] **Step 6: Update `chat_private.rs` — delegate `signed_upload_url`, `verify_upload_signature`, `object_root_path`**

Add delegate methods in `chat_private.rs` (or wherever these are in `impl AppState`):
```rust
    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        self.object_storage.signed_upload_url(document_id, object_path, expires_at_unix)
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        self.object_storage.verify_upload_signature(document_id, object_path, expires, signature)
    }

    pub(crate) fn object_root_path(&self) -> &Path {
        self.object_storage.object_root_path()
    }
```

Also update `resolve_citation_asset_url` in `assets_notifications.rs` to call `self.object_storage.resolve_citation_asset_url(asset).await`.

- [ ] **Step 7: Run `cargo check -p app && cargo check -p transport-http`**

Fix any remaining references to removed fields.

- [ ] **Step 8: Run tests**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app --lib
```

---

## Task 4: Phase 4 — Create BillingContext

**Covers:** ADR §4 (Phase 4: 提取 BillingContext)

**Files:**
- Create: `crates/app/src/billing_context.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/lib_impl/state_types.rs:25,33`
- Modify: `crates/app/src/lib_impl/state_methods.rs`
- Modify: `crates/app/src/lib_impl/chat_private.rs:210-343`

- [ ] **Step 1: Create `billing_context.rs`**

```rust
// crates/app/src/billing_context.rs
use std::sync::Arc;
use avrag_auth::AuthContext;
use common::AppError;
use uuid::Uuid;

#[derive(Clone)]
pub struct BillingContext {
    quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    usage_limit_phase: String,
}

impl BillingContext {
    pub fn new(
        quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
        usage_limit_phase: String,
    ) -> Self {
        Self {
            quota_manager,
            usage_limit_phase,
        }
    }

    pub fn is_available(&self) -> bool {
        self.quota_manager.is_some()
    }

    pub fn usage_limit_phase(&self) -> &str {
        &self.usage_limit_phase
    }

    pub async fn get_user_usage_limit(
        &self,
        auth: &AuthContext,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = auth
            .actor_id()
            .map(|a| a.into_uuid())
            .ok_or_else(|| AppError::internal("no authenticated user"))?;
        let org_id = auth.org_id().into_uuid();
        qm.rolling_service()
            .get_user_usage(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("failed to get usage limit: {}", e)))
    }

    pub async fn check_user_quota(
        &self,
        auth: &AuthContext,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = auth
            .actor_id()
            .map(|a| a.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let org_id = auth.org_id().into_uuid();
        qm.rolling_service()
            .check_quota(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("usage limit check failed: {}", e)))
    }

    pub(crate) async fn ensure_metric_quota(
        &self,
        auth: &AuthContext,
        metric_type: &str,
        requested: i64,
    ) -> Result<(), AppError> {
        if requested <= 0 {
            return Ok(());
        }
        let Some(ref qm) = self.quota_manager else {
            return Ok(());
        };
        let user_uuid = auth
            .actor_id()
            .map(|v| v.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let decision = qm
            .check_quota(
                auth.org_id().into_uuid(),
                user_uuid,
                metric_type,
                requested,
            )
            .await
            .map_err(crate::lib_impl::map_anyhow_error)?;

        if decision.allowed {
            return Ok(());
        }

        let error_message = decision
            .reason
            .as_ref()
            .map(|reason| reason.to_string())
            .unwrap_or_else(|| format!("quota exceeded for {}", metric_type));

        Err(AppError::rate_limited(
            "quota_exceeded",
            error_message,
            decision.retry_after_secs,
        ))
    }

    pub async fn record_llm_usage(
        &self,
        auth: &AuthContext,
        analytics: &crate::analytics_context::AnalyticsContext,
        feature: avrag_billing::usage_limit::BillableFeature,
        stage: &str,
        usage: &avrag_llm::LlmUsage,
        source: &str,
    ) {
        if let Some(ref qm) = self.quota_manager {
            let user_id = auth
                .actor_id()
                .map(|a| a.into_uuid())
                .unwrap_or_else(Uuid::nil);
            let org_id = auth.org_id().into_uuid();
            let ctx = avrag_billing::usage_limit::MeteringContext {
                user_id,
                org_id,
                feature,
                stage: stage.to_string(),
                session_id: None,
                document_id: None,
                request_id: auth.request_id().map(|s| s.to_string()),
                trace_id: None,
            };
            let _ = qm
                .rolling_service()
                .record_usage(
                    &ctx,
                    avrag_billing::usage_limit::UsageRecord {
                        provider: &crate::lib_impl::state_methods::non_empty_or_unknown(&usage.provider),
                        model: &crate::lib_impl::state_methods::non_empty_or_unknown(&usage.model),
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                        usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                    },
                )
                .await;
        }
        analytics
            .record_cost_event(crate::analytics_context::CostEventRecord {
                event_name: analytics::CostEventName::LlmUsageMetered,
                feature: feature.as_str(),
                session_id: None,
                notebook_id: None,
                usage,
                source,
                metadata: serde_json::json!({
                    "stage": stage,
                    "feature": feature.as_str(),
                }),
            })
            .await;
    }
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `pub mod billing_context;` after `pub mod object_storage_context;`.

- [ ] **Step 3: Update `state_types.rs` — replace fields**

Replace lines 25 and 33:
```rust
    pub(crate) quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    ...
    pub(crate) usage_limit_phase: String,
```
With:
```rust
    pub(crate) billing: crate::billing_context::BillingContext,
```

- [ ] **Step 4: Update `state_methods.rs` — constructors**

In both `new()` and `bootstrap()`, construct `BillingContext`:
```rust
        let billing = crate::billing_context::BillingContext::new(
            quota_manager,
            config.usage_limit.enforcement_phase.clone(),
        );
```

Replace field assignments in `Self { ... }`:
- Remove `quota_manager,`
- Remove `usage_limit_phase: config.usage_limit.enforcement_phase,`
- Add `billing,`

Add accessor:
```rust
    pub fn billing(&self) -> &crate::billing_context::BillingContext {
        &self.billing
    }
```

- [ ] **Step 5: Update `chat_private.rs` — delegate billing methods**

Replace the 4 billing methods (`get_user_usage_limit`, `check_user_quota`, `ensure_metric_quota`, `record_llm_usage_if_available`) with delegates:
```rust
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        self.billing.get_user_usage_limit(&self.auth).await
    }

    pub async fn check_user_quota(
        &self,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        self.billing.check_user_quota(&self.auth).await
    }

    pub(crate) async fn ensure_metric_quota(
        &self,
        metric_type: &str,
        requested: i64,
    ) -> Result<(), AppError> {
        self.billing.ensure_metric_quota(&self.auth, metric_type, requested).await
    }

    pub(crate) async fn record_llm_usage_if_available(
        &self,
        feature: avrag_billing::usage_limit::BillableFeature,
        stage: &str,
        usage: &avrag_llm::LlmUsage,
        source: &str,
    ) {
        let analytics_ctx = self.analytics_ctx();
        self.billing.record_llm_usage(&self.auth, &analytics_ctx, feature, stage, usage, source).await
    }
```

- [ ] **Step 6: Update `chat/service_postprocess.rs` — billing usage**

In `service_postprocess.rs` (line 250), replace direct `self.quota_manager` access with `self.billing.quota_manager()` or use the delegate. Since `service_postprocess.rs` directly accesses `self.quota_manager`, add a method to `BillingContext`:

```rust
    pub fn quota_manager(&self) -> Option<&Arc<avrag_billing::QuotaManager>> {
        self.quota_manager.as_ref()
    }
```

Then change `self.quota_manager` → `self.billing.quota_manager()` in `service_postprocess.rs`.

- [ ] **Step 7: Run `cargo check -p app && cargo check -p transport-http`**

Fix any remaining references to `self.quota_manager` or `self.usage_limit_phase`.

- [ ] **Step 8: Run full test suite**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app --lib
```

---

## Task 5: Phase 2–4 — Verify and Fix All Callers

**Covers:** ADR §7 (验证标准)

**Files:**
- Modify: any remaining files with direct field access

- [ ] **Step 1: Run `cargo check -p app`**

Fix all compilation errors. Common patterns:
- `self.llm_client` → `self.llm_ctx.agent_client()`
- `self.memory_llm_client` → `self.llm_ctx.memory_client()`
- `self.object_store` → `self.object_storage.object_store()`
- `self.public_base_url` → `self.object_storage.public_base_url()`
- `self.quota_manager` → `self.billing.quota_manager()`
- `self.usage_limit_phase` → `self.billing.usage_limit_phase()`

- [ ] **Step 2: Run `cargo check -p transport-http`**

Fix any transport-http compilation errors.

- [ ] **Step 3: Run full test suite**

```bash
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app --lib
cd /home/chuan/context-osv6/avrag-rs && cargo test -p app --test product_e2e product_e2e::smoke
```

- [ ] **Step 4: Verify field count**

After all changes, AppState should have ~13 fields:
```rust
pub struct AppState {
    pub(crate) auth: AuthContext,
    pub(crate) pg: Option<Arc<PgAppRepository>>,
    pub(crate) inner: Arc<RwLock<MemoryState>>,
    pub(crate) llm_ctx: LlmContext,
    pub(crate) chatmemory: Option<Arc<ChatMemory>>,
    pub(crate) analytics: Option<Arc<analytics::AnalyticsService>>,
    pub(crate) billing: BillingContext,
    pub(crate) rag_runtime: Option<Arc<RagRuntime>>,
    pub(crate) agent_service: Option<Arc<UnifiedAgentService>>,
    pub(crate) object_storage: ObjectStorageContext,
    pub(crate) guard_pipeline: Arc<GuardPipeline>,
    pub(crate) uses_memory_adapters: bool,
    pub(crate) search_provider: String,
    pub(crate) search_mode: String,
    pub(crate) redis_url: String,
    pub(crate) max_upload_file_size_bytes: u64,
    pub(crate) api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    pub(crate) key_vault: Arc<dyn KeyVault>,
}
```

That's 18 fields (down from 22). The 4 removed fields are: `llm_client`, `memory_llm_client` (→ `llm_ctx`), `object_store`+`public_base_url`+`object_root`+`object_storage_upload_expire_sec`+`object_storage_download_expire_sec` (→ `object_storage`), `quota_manager`+`usage_limit_phase` (→ `billing`).

Net: 22 → 18 fields (7 removed, 3 new context fields added).

- [ ] **Step 5: Verify no `#[allow(dead_code)]` on new code**

New context structs should not need `#[allow(dead_code)]`. Legacy delegates may temporarily have it (acceptable per ADR).

---

## Notes

- **Legacy delegates preserved:** All old `AppState` methods (`signed_upload_url`, `get_user_usage_limit`, etc.) are kept as thin delegates. This ensures backward compatibility — no external caller needs to change in this phase.
- **`map_anyhow_error` visibility:** `BillingContext` needs access to `map_anyhow_error` from `chat_private.rs`. Make it `pub(crate)` if not already.
- **`sign_upload_payload` / `upload_signing_secret` visibility:** `ObjectStorageContext` needs these from `chat_private.rs`. Make them `pub(crate)`.
- **Phase 5 is separate:** The ADR's Phase 5 (StorageContext + OrchestratorContext) is a much larger effort (47 files) and is NOT included in this plan.
