# DEFERRED-2 (W4e) — Decompose `StorageContext`

> Thermo-Nuclear review finding **B4 (CRITICAL)**. Split out as a standalone
> task because it is the highest-risk item in the backlog (the plan rated it
> "极高风险", 3–5 days, 113 references).
>
> **Status:** Steps 0–2 done (2026-07-09). Step 3 optional / deferred.
> **Prerequisite:** workspace green, all M3 struct splits stable (met as of
> 2026-07-09).

---

## 1. Problem

`crates/app-core/src/storage_context.rs:25` — `StorageContext` is a 19-field
god-bag mixing four unrelated concerns behind one 19-positional-argument
constructor:

| # | Field | Concern |
|---|-------|---------|
| 1 | `postgres_health: Option<Arc<dyn PostgresHealthPort>>` | Infra |
| 2 | `postgres_configured: bool` | Infra |
| 3 | `document_store: Option<Arc<dyn DocumentStorePort>>` | Domain store |
| 4 | `auth_store: Option<Arc<dyn AuthStorePort>>` | Domain store |
| 5 | `admin_store: Option<Arc<dyn AdminStorePort>>` | Domain store |
| 6 | `billing_quota: Option<Arc<dyn BillingQuotaPort>>` | Domain store |
| 7 | `billing_store: Option<Arc<dyn BillingStorePort>>` | Domain store |
| 8 | `share_store: Option<Arc<dyn ShareStorePort>>` | Domain store |
| 9 | `chat_persistence: Option<Arc<dyn ChatPersistencePort>>` | Domain store |
| 10 | `inner: Arc<RwLock<MemoryState>>` | In-memory state |
| 11 | `api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>` | In-memory state |
| 12 | `api_key_hashes: Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>` | In-memory state |
| 13 | `max_upload_file_size_bytes: u64` | Infra |
| 14 | `uses_memory_adapters: bool` | Infra |
| 15 | `object_store: Arc<dyn ObjectStorePort>` | Object-store config |
| 16 | `public_base_url: String` | Object-store config |
| 17 | `object_root: String` | Object-store config |
| 18 | `upload_expire_sec: u64` | Object-store config |
| 19 | `download_expire_sec: u64` | Object-store config |

Two static helpers (`current_org_id`, `current_user_id`) are also defined on
`StorageContext` but only touch `AuthContext` — they do not belong here.

---

## 2. Current-state facts (measured 2026-07-09)

- **References:** `StorageContext` appears in **113 occurrences / 22 files**.
- **Construction sites:** exactly **2**, both in `app-bootstrap/src/lib.rs`
  (`:136` memory mode, `:345` postgres mode). Both build the full 19-arg list.
- **Owners:** held as a cloned value field on
  - `AppState.storage` (`app-bootstrap/src/app_state/state_types.rs:13`)
  - a public config struct (`app-bootstrap/src/lib.rs:67`)
  - `ChatContext.storage` (assigned via `self.chat.storage = self.storage.clone()`
    in `state_methods.rs:114`)
- **Mutability:** `set_uses_memory_adapters(&mut self, bool)` is called at
  runtime (`state_methods.rs:113`) and in one test — the struct is NOT immutable.
- **Accessor heatmap** (call sites outside `storage_context.rs`):

  | Group | Accessor | Calls |
  |-------|----------|------:|
  | Domain store | `document_store` | 24 |
  | Domain store | `chat_persistence` | 23 |
  | Domain store | `share_store` | 21 |
  | Domain store | `admin_store` | 15 |
  | Domain store | `auth_store` | 14 |
  | Domain store | `billing_store` | 10 |
  | Domain store | `billing_quota` | 6 |
  | In-memory | `inner` | many (hottest field) |
  | In-memory | `api_keys` / `api_key_hashes` | 7 / 6 |
  | Infra | `postgres_configured` | 12 |
  | Infra | `uses_memory_adapters` / `max_upload_file_size_bytes` | 7 / 6 |
  | Infra | `runtime_mode` / `pg_ready` | 5 / 3 |
  | Object-store | `object_store` | 4 |
  | Object-store | `signed/verify_upload_signature`, `resolve_citation_asset_url` | 2 each |
  | Object-store | `*_expire_sec`, `public_base_url`, `object_root_path` | 1–2 each |

---

## 3. Why this is risky — and the strategy that defuses it

The original plan proposed field-group sub-structs **and** updating all 113
references crate-by-crate. That is the dangerous path: every consumer edit is a
chance to break the persistence/auth/object layers, and there is no incremental
green checkpoint until the whole workspace is touched.

**The safe strategy is "facade + internal grouping":** extract the field
groups into sub-structs *inside* `StorageContext`, but **keep every existing
accessor signature identical**. Consumers keep calling
`storage.document_store()`; internally that becomes `self.stores.document_store()`.
**Zero caller changes** for the bulk of the work.

Only two things actually hurt today:
1. The 19-positional constructor (a real bug-magnet — easy to swap two `bool`/`u64` args).
2. Four unrelated concerns fused in one type (cognitive / testing cost).

Both are fixable **without** touching the 113 reference sites.

---

## 4. Execution plan

Each step is independently shippable and must pass the verification gate before
the next begins.

### Step 0 — Move the static helpers (warmup, ~15 min, zero risk)

`current_org_id` / `current_user_id` take `&AuthContext` and never touch
instance state. Move them to free functions in `app-core` (e.g.
`auth_scope::current_org_id`) and replace the ~6 callers
(`app-documents/src/{notebooks,url_imports}.rs`, `app-core/src/adapters/memory.rs`).

- Verify: `cargo build --workspace && cargo test --workspace --lib`

### Step 1 — Kill the 19-positional constructor (~2 h, low risk)

Replace `StorageContext::new(19 positional args)` with a grouped builder:

```rust
pub struct StorageContextParts {
    pub infra: StorageInfra,
    pub stores: StorageStores,
    pub memory: MemoryStateHandles,
    pub objects: ObjectStoreConfig,
}
impl StorageContext {
    pub fn from_parts(parts: StorageContextParts) -> Self { ... }
}
```

The 4 sub-structs are defined now (even if their fields are later hoisted in
Step 2). Update the **2** construction sites in `app-bootstrap/src/lib.rs` to
build `StorageContextParts`. The named-field struct literal makes arg-order
bugs impossible and self-documents each group.

- Verify: `cargo build --workspace && cargo test --workspace --lib --bins`
- Gain: constructor is no longer a footgun; the 4 concerns are named.

### Step 2 — Hoist fields into the sub-structs behind the facade (~3–4 h, low risk)

Move the 19 fields off `StorageContext`'s flat layout into the 4 sub-struct
fields. **Keep every `pub fn document_store()` / `object_store()` / `inner()` /
… accessor with its current signature** — they now read through the sub-struct.
This is a purely mechanical edit confined to `storage_context.rs`.

Groups (matches the original plan):

- `StorageStores` — `document_store`, `auth_store`, `admin_store`,
  `billing_quota`, `billing_store`, `share_store`, `chat_persistence` (7)
- `StorageInfra` — `postgres_health`, `postgres_configured`,
  `uses_memory_adapters`, `max_upload_file_size_bytes` (4)
- `ObjectStoreConfig` — `object_store`, `public_base_url`, `object_root`,
  `upload_expire_sec`, `download_expire_sec` (5) + the
  `signed_upload_url` / `verify_upload_signature` / `resolve_citation_asset_url`
  methods move onto it
- `MemoryStateHandles` — `inner`, `api_keys`, `api_key_hashes` (3)

Handle the one mutator: `set_uses_memory_adapters` delegates to
`self.infra.set_uses_memory_adapters(..)` (keep `&mut self` on the facade so
AppState is unchanged).

- Verify: `cargo build --workspace && cargo test --workspace --lib --bins && cargo clippy -p app-core -p app-bootstrap`

At this point **the god-bag is decomposed and no consumer was touched.** This
is the recommended stopping point for the risk budget.

### Step 3 — (optional) Narrow consumer dependencies (~1 day, medium risk)

Only attempt after Step 2 has baked. Expose the sub-struct accessors
(`storage.objects()`, `storage.infra()`) and migrate the handful of consumers
that use **only** one group to depend on the narrower type — e.g. the
citation/upload-signing handlers only need `&ObjectStoreConfig`, the worker's
health check only needs `&StorageInfra`. Each migration is independent and
reversible.

- Verify per consumer: `cargo test -p <crate>`

---

## 5. What NOT to do

- **Do not** update the 113 reference sites as a precondition. The facade keeps
  them valid; touching them en masse is where the extreme risk lives.
- **Do not** change accessor signatures in Step 2 — that is what would force a
  cascade through AppState/ChatContext/handlers.
- **Do not** remove `#[derive(Clone)]` — `AppState` and `ChatContext` both hold
  cloned copies (`state_methods.rs:114`).

---

## 6. Verification gate (run after every step)

```bash
# in avrag-rs/
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy -p app-core -p app-bootstrap -p app-chat -p transport-http -- -D warnings
```

## 7. Definition of done (recommended scope = Steps 0–2)

- [x] `StorageContext::new(…)` takes no positional args (grouped parts only).
- [x] The 19 fields live on 4 focused sub-structs.
- [x] All existing accessors unchanged — facade keeps callers on
      `storage.document_store()` etc. Construction sites (2 production + tests)
      updated to `from_parts`; no mass rewrite of the 113 accessor references.
- [x] Static helpers relocated out of `StorageContext` (`auth_scope`).
- [x] Workspace build green; relevant lib/integration tests green.
      (`cargo clippy -p app-core --no-deps -D warnings` green;
      app-bootstrap has pre-existing clippy debt unrelated to this task.)
