Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# Product App Architecture (backend `avrag-rs`) — **mandatory for new work**

**Status (2026-07-10):** Phase A–C **Done** (product entry + Write/Agent lanes + wrapper slim). TN review: **APPROVE**. Residual cleanup Done: [`docs/engineering/PRODUCT_APP_RESIDUAL_CLEANUP_PLAN_2026-07-10.md`](../engineering/PRODUCT_APP_RESIDUAL_CLEANUP_PLAN_2026-07-10.md). Full history: ADR-0007, `PRODUCT_APP_*` plans under `docs/engineering/`.

## Current shape (do not regress)

```text
Transport / MCP (thin: parse, auth, status codes)
        │
        ▼
Product Apps (app-bootstrap/src/product_apps/)
  conversation()  → sole chat/rag/search/write EXECUTE entry
  agent()         → sessions / search / citations / runtime_tools / usage
  workspace()     → workspaces / documents / sources
  share() / billing_api() / prefs() / admin_api() / admin_ops()
        │
        ▼
Domain crates (app-chat, write-core, share, agent-tools, …)
  write lane  → execute_write_pipeline → run_write_mode  (NOT ToolCatalog)
  agent lane  → execute_chat_pipeline → dispatch_agent_mode + ToolCatalog/dispatch_tool
```

AppState is a **composition root + face factory** (still holds fat infra contexts). **Do not** add new business methods on `AppState`. Put use-cases on the right `*App` or in domain crates.

## Iron rules (T1–T8) — single definition

| # | Rule |
|---|------|
| T1 | **No new business methods** on `AppState` or shallow faces; new capability → domain service / target Product App |
| T2 | **Write forever outside ReAct ToolCatalog**; `write_refine_*` only via `write_refine::tool_specs_for_pool` (Write control ring) |
| T3 | Chat/RAG/Search tool **execute** only through `ToolCatalog` / `dispatch_tool` |
| T4 | **No C4**: Capability / Skill / Tool stay three layers (ADR-0006 §5a) |
| T5 | Behavior-preserving slices; daily verify with **L1** (`bash scripts/test-l1.sh` or targeted `cargo test -p …`) |
| T6 | Solo local trunk; do not expand CI theater for architecture work |
| T7 | **`workspace` is the sole product truth** replacing `notebook` (see below) |
| T8 | **No product `org`**: tenant/ownership is **`user_id` / `owner_user_id`**, scope is **`workspace_id`**. Migration in progress — **do not add new org surface area** |

## Workspace supersedes notebook (sole source of truth)

**Canonical product term: `workspace`.** `notebook` is a **legacy alias only** (pre-rename residual). Do **not** reintroduce `notebook` as the primary name.

| Surface | Required |
|---------|----------|
| API / JSON / tool schemas / error messages / new tests | Prefer **`workspace`** (`workspace_id`, `scope=workspace`, `WorkspaceApp`, …) |
| Domain enums / Product Apps | **`Workspace`**, `state.workspace()` — never new `Notebook*` product APIs |
| Wire labels (tool results, SSE, registry) | Emit **`workspace`**, not `notebook` |
| Incoming legacy values | May **accept** `notebook` as a one-way alias → map to workspace; **never** invent new notebook-first paths |
| Local vars in old tests | Fine if unexported; **do not** copy into new public contracts or mock tool args as the preferred spelling |

* When a test or mock fails because product returns `workspace` and the test expected `notebook` (or the reverse): **fix product/schema/wire toward `workspace`**, not "align tests back to notebook."
* Related residual notes: [`docs/engineering/WORKSPACE_RENAME_DECISIONS_2026-07-09.md`](../engineering/WORKSPACE_RENAME_DECISIONS_2026-07-09.md) if present.

## Org removed as product/tenant concept (sole source of truth)

**Product is B2C personal:** account (`user`) + **`workspace`**. There is **no** team/organization product concept.

| Surface | Required |
|---------|----------|
| New API / JSON / MCP tools / tests / error copy | **Never** introduce `org_id`, `OrgId`, `organizations`, `x-org-id`, `app.current_org`, MCP `org.*` |
| Ownership / RLS / isolation | Prefer **`owner_user_id`** or **`user_id`**; resource scope **`workspace_id`** |
| Auth context (target) | Root = **user**; optional workspace scope — **not** org |
| Admin | Users / usage only — **no** new Organizations admin features |
| Existing residual `org_*` in schema/code | **Migrate off** per plan; if a test fails because product still has `org_id` while new code uses owner user: **fix toward user/workspace**, never "align new code back to org" |

* Full plan (ingestion fix + org hard-cut waves): [`docs/engineering/INGESTION_AND_ORG_REMOVAL_UNIFIED_PLAN_2026-07-10.md`](../engineering/INGESTION_AND_ORG_REMOVAL_UNIFIED_PLAN_2026-07-10.md).
* Billing already user-scoped (ADR-0001 / migration 0035). Do **not** reintroduce org-level subscription keys.
* Until O-wave Done, reading legacy columns is fine; **writing new org-first APIs is forbidden**.

## Coding standards for features

* **Execute path:** handlers/MCP call **`state.conversation().execute` / `execute_stream` only**. No `if agent_type == "write"` in transport; no `state.chat().execute_*` for product execute.
* **Sessions / search / citations:** `state.agent().…` (not raw `ChatContext` in new production code).
* **Documents / workspaces:** use `state.workspace()` for documents/workspaces (**not** notebook APIs).
* **Do not** add new Product App types or pass-through wrappers "for architecture." Deletion test: if removing the type only forces callers to use the inner type, delete it.
* **Do not** re-register `write_refine_*` on SkillRegistry / ToolCatalog or restore meta side-tables.
* **Domain depth:** business logic lives in domain crates (`app-chat`, `write-core`, `avrag_share`, …). Product Apps orchestrate; they must not become a second copy of Bound god-objects.
* **AppState is composition root + face factory; product API is Product Apps only.** Residual plan Done.

## Verification defaults

* After touching product entry / pipeline / tools: `cargo test -p app-bootstrap --lib`, `cargo test -p app-chat --lib`, `cargo test -p agent-tools --lib` as relevant; wave end or ask → full L1 (`bash scripts/test-l1.sh`).
* **WSL resource defaults:** L1 and `avrag-rs/.cargo/config.toml` cap `jobs=2` / modest test threads. Override with `CARGO_BUILD_JOBS` / `L1_TEST_THREADS` or `local-machine.toml`. Do not stack concurrent full `cargo test` runs.
* Real LLM / full Playwright: **not** required to land architecture or mid-wave product features.
