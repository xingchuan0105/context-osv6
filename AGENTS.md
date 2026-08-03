# Agent rules — context-osv6

## Precedence

1. User's explicit request for this turn
2. This repo's hard rules (Product T1–T8, **prompts-in-md**, **third-person observation not orders**, no golden-set leakage, workspace/org, `.env` reuse, solo trunk, graphify update after structural edits, deploy scripts only, service assumptions)
3. Generic style preferences

## Behavior (project deltas only)

- State assumptions explicitly; **stop and ask** when a request is ambiguous. Push back with a simpler option when warranted.
- Surgical edits: every changed line traces to the user's request. Match local style. Remove only unused symbols **you** introduced — **do not delete pre-existing dead code unless asked**.
- Multi-step work: brief plan with verify gates; do not advance past a failing gate.

## Prompts — non-negotiable (`avrag-rs`)

### Location

**Forbid hardcoding LLM-facing prompt prose in Rust (or other product code).** Author and edit it only under `avrag-rs/prompts/**/*.md`; runtime may `include_str!` / load / substitute placeholders and assemble — never invent Chinese/English instruction bodies in source.

| Do | Don't |
|----|--------|
| Capability copy in `prompts/capabilities/<id>/{contract.md,SKILL.md,reference/}`（knowledge-base / web）；skill / orchestrator / synthesis copy in `prompts/clusters/`（docscope / memory / writing / format / heavytail-* / index / workspace-create）、`prompts/agent-guide/`、`prompts/system/hints/`、`prompts/templates/` | Multi-line instruction strings inline in `agent-loop` / app crates |
| Loop **observations** (no-chunk, budget C5, sandbox_error, format_hint, …) in `prompts/loop/*.md` via `react_loop/prompt_assets.rs` | New observation/repair/fallback sentence left inline next to control flow |
| Placeholders only in code (`{n_blocks}`, `{tool}`, …) | Paste realistic-corpus / golden-set queries, gold answers, entity names, or eval numbers into prompts, loop code, or unit-test fixtures that claim to be product policy |
| SDK/tool **observation data** (stdout, retrieval JSON) as runtime feedback | Treat tool stdout as a place to author system instructions |

**Exceptions (not “prompt authoring”):** pure control tokens (`exit_reason` ids), regex/match keyword lists used as detectors (not injected as instructions), short UI progress labels, machine-stable error codes. If text is **shown to the model as instruction or user-turn guidance**, it belongs in `prompts/`.

**Host-observation markers must be registered first.** Any tag the host injects into the model context (loop observations, budget hints, cluster indices, `[retrieval_summary]`, …) must be registered in `avrag-rs/crates/agent-loop/src/react_loop/host_markers.rs` before first use — emitters reference the constant, detectors derive from the table, and a parity test fails on any unregistered tag in `prompts/loop/*.md`.

Layout map: `avrag-rs/prompts/README.md`. Loop assets: `avrag-rs/prompts/loop/README.md`.

### Voice: third-person observation, not orders (LLM autonomy)

**All LLM-facing prompts** (capability/skills, answer-phase check/observation, loop nudges, synthesis repair, format hints, …) are written as **third-person narrative of what happened or what is true in the environment** — not as a to-do list for the model.

| Prefer（发生了什么 / 环境是什么） | Avoid（命令 / 禁令 / 步骤） |
|----|----|
| 「本轮检索观察中仍未出现 answer-grade 命中。」 | 「禁止终答。请继续用 client 检索。」 |
| 「草稿里问题侧 A 有 observation 支撑；侧 B 仍未见命中。」 | 「请再写一个 code 块补检 B。」 |
| 「管道表中一行是一条记录；`total_hits` 是命中行数。」 | 「应/必须/不要/禁止 dedupe。」 |
| 「沙箱本轮 stdout/stderr 为空，且未发生 client.* 调用。」 | 「请检查代码路径并修复。」 |
| Few-shot：情境 → observation → 读出的事实 | 「正确做法是：先…再…」 |

**Goal:** maximize model agency. The runtime **reports state** (budget, empty evidence, mechanism facts); the model **decides** the next action. Do not smuggle a second policy layer of “you must / must not” into prose when a hard gate already exists in code.

**Scope:** skill bodies, capability manuals, `prompts/loop/*` observations, any other string assembled into the model context as guidance. **Out of scope for this voice rule:** short end-user product copy that is the final answer shown to a human (degraded empty-result lines), pure machine tags, detector keyword lists.

### Stop decision (who may end the retrieve loop)

Aligned with single-agent / pi-style **agentLoop**: after tools/codegen, **whether to stop is model + skill**. Host does **not** run semantic “coverage / completeness” heuristics to refuse `DirectAnswer`.

| Term (prefer) | Meaning | Owner |
|---------------|---------|--------|
| **Continue** | Next retrieve turn (codegen/tools/skill_request) | Model chose tools/code, or host structural gate |
| **DirectAnswer** / **stop** | Final prose; loop ends (no further tool turn) | **Model + skill** (with evidence present) |
| **observation** | Host-injected user message stating runtime facts (`prompts/loop/*`) | Host reports; model acts |
| **compile_feedback** | Free correction turn after **structural** handoff compile fail (worker path only) | Host structural only |
| **require_evidence** | Product/skill **intent** that grounded facts come from observation — **not** a host hard gate | **Skill + model** only |
| **compile_feedback** | Free structural correction (worker handoff) | Host structural only |
| **token / round budget** | Primary stop when cost ceiling hit | Host cost policy |

**Do not:** host-side claim checklists, multi-entity scanners, soft-refusal keyword bars, or **no-chunk refuse DirectAnswer** for `require_evidence`. Grounding and multi-claim coverage live in **skill/capability prose** (third-person), not `exit_policy` enforcement.

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
- Long E2E runs (full-149, staging ingest, DR2/L3): never block on one global timeout — background + log file + poll progress; use `scripts/with-watchdog.sh` (silence kill) and `scripts/test-full149.sh` (canonical full-149, circuit breaker `E2E_ABORT_AFTER_CONSECUTIVE_FAILS=8`). Full conventions: `avrag-rs/docs/e2e-gates.md` §Agent run conventions.

## Repo map

- Root WSL: `/home/chuan/context-osv6` · Windows: `Z:\home\chuan\context-osv6`
- Product backend: `avrag-rs` · Frontend: `frontend_next` (Next.js + React + TS, pnpm) · `frontend_rust` workspace only when explicitly asked.

## More (links only)

- `docs/agent/product-apps.md` — full T1–T8 / workspace / org text
- `avrag-rs/prompts/README.md` · `avrag-rs/prompts/loop/README.md` — prompt CDS + loop nudge load path
- `docs/agent/graphify.md` — graphify query & update rules
- `docs/agent/wsl-services.md` — services, ports, VPS
- `docs/agent/rust-resources.md` — target/cache policy
- `docs/agent/coding-behavior.md` — original long-form behavior essays (human reference)
- `docs/engineering/SOLO_DISCIPLINE.md` · `docs/adr/0007-product-apps-composition-root.md`
