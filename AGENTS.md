# Agent rules — context-osv6

## Precedence

1. User's explicit request for this turn
2. This repo's hard rules (Product T1–T8, workspace/org, `.env` reuse, solo trunk, graphify update after structural edits, deploy scripts only, service assumptions)
3. Generic style preferences

## Behavior (project deltas only)

- State assumptions explicitly; **stop and ask** when a request is ambiguous. Push back with a simpler option when warranted.
- Surgical edits: every changed line traces to the user's request. Match local style. Remove only unused symbols **you** introduced — **do not delete pre-existing dead code unless asked**.
- Multi-step work: brief plan with verify gates; do not advance past a failing gate.

## Product hard rules (`avrag-rs`) — non-negotiable (formerly §8)

| # | Rule (single-line form; full text: `docs/agent/product-apps.md`) |
|---|------|
| T1 | No new business methods on `AppState` / shallow faces → domain service or target Product App |
| T2 | Write forever outside ReAct ToolCatalog; `write_refine_*` only via `write_refine::tool_specs_for_pool` |
| T3 | Chat/RAG/Search tool execute only via `ToolCatalog` / `dispatch_tool` |
| T4 | No C4: Capability / Skill / Tool stay three layers |
| T5 | Behavior-preserving slices; verify with targeted `cargo test -p …` / L1 |
| T6 | Solo local trunk; no CI theater |
| T7 | `workspace` is the sole product truth (never new notebook-primary APIs) |
| T8 | No product `org`: ownership `user_id`/`owner_user_id`, scope `workspace_id` |

- Execute only via `state.conversation().execute` / `execute_stream`. Sessions/search/citations via `state.agent()`. Documents/workspaces via `state.workspace()`.
- Fix failures **toward** `workspace` / `user`, never "align back" to `notebook` / `org`.

## Code intelligence

- Structure / relations / blast radius: **graphify first** (MCP `query_graph` etc. with `project_path: "/home/chuan/context-osv6"`, or CLI `graphify query|path|explain`). Full rules: `docs/agent/graphify.md`.
- Semantic chunk search: `semble`; exact literal strings: `grep` last.
- **After structural code changes, run `graphify update .` (or `--force`) in the same session before claiming done.** Never commit `graphify-out/`.

## Environment & solo

- Credentials: always read `avrag-rs/.env` (+ `.env.example`) first; **reuse configured values silently, never re-ask**; persist user-supplied new values to `.env` incrementally.
- Default: local trunk `master`; commit locally. No push / PR / CI babysitting unless the user asks. Full discipline: `docs/engineering/SOLO_DISCIPLINE.md`.
- Services (Milvus/PG/Redis/MinIO): assume running per `docs/agent/wsl-services.md`; do not `docker-compose up` blindly; do not prune `avrag-test-pg-*` containers.
- Deploy: only `scripts/deploy-*.sh` (status: `scripts/deploy-status.sh`). Never ad-hoc ssh/scp product code from chat.

## Verify defaults

- Touched Rust: `cargo test -p <pkg> --lib` as relevant (`app-bootstrap`, `app-chat`, `agent-tools`, …); wave end or on request → `bash scripts/test-l1.sh`.
- Frontend (`frontend_next`): `pnpm test` / typecheck.
- WSL: respect `jobs=2`; never stack concurrent full `cargo test` runs. Details: `docs/agent/rust-resources.md`.
- Real LLM / full Playwright: not required mid-wave (E2E semantics: `avrag-rs/docs/e2e-gates.md`).

## Repo map

- Root WSL: `/home/chuan/context-osv6` · Windows: `Z:\home\chuan\context-osv6`
- Product backend: `avrag-rs` · Frontend: `frontend_next` (Next.js + React + TS, pnpm) · `frontend_rust` workspace only when explicitly asked.

## More (links only)

- `docs/agent/product-apps.md` — full T1–T8 / workspace / org text
- `docs/agent/graphify.md` — graphify query & update rules
- `docs/agent/wsl-services.md` — services, ports, VPS
- `docs/agent/rust-resources.md` — target/cache policy
- `docs/agent/coding-behavior.md` — original long-form behavior essays (human reference)
- `docs/engineering/SOLO_DISCIPLINE.md` · `docs/adr/0007-product-apps-composition-root.md`
