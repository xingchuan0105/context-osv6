# Backend agent rules — avrag-rs

Scope: `avrag-rs/`. Inherits [repository rules](../AGENTS.md). Paths below are relative to this directory unless stated otherwise.

## Product architecture

Before changing product APIs, ownership, entry points, pipelines, or tools, read [Product Apps and T1–T8](../docs/agent/product-apps.md), the single definition of the backend architecture rules.

- `AppState` is a composition root; business logic belongs in domain services or the appropriate Product App.
- Execute through `state.conversation().execute` / `execute_stream`; sessions/search/citations use `state.agent()`; documents/workspaces use `state.workspace()`.
- Chat/RAG/Search tools execute through `ToolCatalog` / `dispatch_tool`. Write stays outside the ReAct ToolCatalog; `write_refine_*` comes only from `write_refine::tool_specs_for_pool`.
- Capability / Skill / Tool remain three layers. Fix toward `workspace` / `user`, never toward `notebook` / `org`.

## Prompts and agent runtime

Before editing model-facing assets, prompt assembly, host observations, stop decisions, or retrieval orchestration, read [LLM guidance](../docs/agent/llm-guidance.md). This describes product agents, not how the coding assistant delegates work.

- LLM-facing prose lives in `prompts/**/*.md`; code may load, substitute, and assemble it. Voice is third-person environment observation.
- Register injected host tags in `crates/agent-loop/src/react_loop/host_markers.rs` before use; emitters use constants and detectors derive from the registry.
- Product `rag` / `search` / dual retrieval uses Lead + specialized Workers, with no independent verify LLM. Lead owns coverage and user synthesis; the host enforces structural gates.
- Host observations and runtime diagnostics stay in the model/telemetry channel, outside the user's main answer.

## Subtex

For `subtex-*` crates, `subtexd`, or `subtex.*` tools, read the [PRD](../docs/plans/2026-09-06-directory-plugin-prd.md) and [prompt/tool index](prompts/subtex/README.md). Consult the [development plan](../docs/plans/2026-09-06-subtex-m1-dev-plan.md) for wave progress and acceptance gates.

- The user's project directory is the sole master; indexes cannot become ingest copies or workspace/notebook/global-KB surfaces.
- Inbox moves require confirmation and stay within attached roots.
- Main crates: `subtex-core`, `subtex-index`, `subtex-store-sqlite`; binary: `subtexd`. Tool registration uses `AVRAG_SUBTEX=1` and a local user token.
- Index storage: `SUBTEX_DATA_DIR` (default `~/.local/share/subtex`); inbox configuration: `inbox.json`; global ledger: `global.db`.

## Verification

Follow the root time-cost rule before runs.

- For Rust code changes, run relevant `cargo test -p <pkg> --lib` from `avrag-rs/` (for example `app-bootstrap`, `app-chat`, `agent-tools`).
- Wave end or when requested: `bash scripts/test-l1.sh` **from the repository root**, after duration/approval requirements are satisfied.
- Respect `jobs=2`; do not stack full Cargo runs. Read [resource policy](../docs/agent/rust-resources.md) before changing targets or caches.
- Real LLM / full Playwright are not required mid-wave. For long E2E, use background execution, logs, progress polling, and the watchdog/circuit-breaker runners in [E2E conventions](docs/e2e-gates.md#agent-run-conventions-long-tasks).
