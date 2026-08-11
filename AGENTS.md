# Agent rules — context-osv6

## Precedence

1. User's explicit request for this turn
2. **Design principles** (below) — when they conflict with older local habits or generic style, **these win**
3. This repo's hard rules (Product T1–T8, **prompts-in-md**, **third-person observation not orders**, no golden-set leakage, workspace/org, `.env` reuse, solo trunk, code-review-graph update after structural edits, deploy scripts only, service assumptions)
4. Generic style preferences

## Design principles (authoritative)

These replace softer “keep compat / leave dead paths / temporary stopgap” habits when they conflict.

- **No backward compatibility tax.** Do not preserve backward compatibility. Remove obsolete paths instead of adding compatibility layers, fallbacks, or migrations.
- **Simplest that fully works.** Choose the simplest implementation that fully meets the current requirements. Avoid speculative abstractions, configuration, and indirection.
- **Layered growth.** Grow the system in layers. Start from the smallest version that works end to end, and add each new capability on top of a product that already works. Never trade a working product for unfinished complexity.
- **Modular separation.** Keep components modular and concerns clearly separated.
- **Prefer proven libraries.** Prefer established, well-maintained libraries when they reduce overall complexity or improve reliability. Do not reimplement common functionality without a clear reason.
- **Reuse project deps first.** Lean on the dependencies already in the project before writing your own implementation or adding packages. Do not assume a library lacks a capability without checking its documentation and types.
- **Long-term architecture.** Make architectural decisions for the long term. Do not accept a stopgap that only works for now and is meant to be replaced later.

## Behavior (project deltas only)

- State assumptions explicitly; **stop and ask** when a request is ambiguous. Push back with a simpler option when warranted.
- Surgical edits: every changed line traces to the user's request. Match local style. When replacing a path, **delete the obsolete path** (no compat shim); do not drive-by delete unrelated code outside the request.
- Multi-step work: brief plan with verify gates; do not advance past a failing gate. Prefer a thin end-to-end slice that works, then layer — never ship unfinished complexity in place of a working product.
- **Time-cost consent:** before any compile or script run (`cargo build/test`, `pnpm`, deploy scripts, E2E, …), estimate the time cost and get user approval first; never launch long-running commands unannounced — keep the dev rhythm predictable.

## Product IA & navigation (frontend)

Authoritative map: **`docs/design/PRODUCT_IA.md`**. Audit notes: `docs/design/PRODUCT_IA_AUDIT.md`. Multi-site **discovery** only: `frontend_next/lib/site-map.ts` (do not treat as in-app IA).

1. **IA before pages.** Before adding/changing global nav, top-bar entries, shells, or monetization entry points in `frontend_next`, update `PRODUCT_IA.md` (Jobs / Sitemap / Canonical / Shell). Do not invent a third way to complete the same user task.
2. **Canonical routes only.** Membership checkout → `/pricing`; wallet top-up → `/pricing#topup`; BYOK → `/settings?tab=providers`; client → `/desktop`. Other CTAs may deep-link; they must not implement a second checkout path. Upgrade modals are marketing explainers, not payment hosts.
3. **Help ≠ primary nav.** Onboarding / product-map content is a modal or `/help`, opened from weak entries (e.g. top-bar「上手」). Never ship a permanent business sidebar of encyclopedia topics beside the workspace list.

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

**Scope:** skill bodies, capability manuals, `prompts/loop/*` observations, any other string assembled into the model context as guidance. **Out of scope for this voice rule:** pure machine tags, detector keyword lists. User-facing disaster fallbacks (if any) live under `prompts/loop/disaster/` and are **not** host footnotes on a model draft.

### User channel: LLM front stage; harness never speaks as the answer

**三角关系：** 用户与 LLM 在前台；harness 是环境（工具执行、第三人称 observation、内部状态机、telemetry）。LLM 可与 harness 多轮交互，中间产物可存；**不得**把协议残片、host observation 外壳、运行时诊断句拼进用户主气泡。

| Do | Don't |
|----|--------|
| 用户主气泡 = 模型自然语言（说明 / 澄清 / 追问均可，措辞由模型自主） | Host 在 answer 字符串后拼接 disclosure / ceiling /「本 run…」类脚注 |
| 失败在环内消化；到顶后 **LLM 收束轮**（仍有 token）或 **极窄灾难兜底句**（token 尽 / 格式闸耗尽） | 「审不过仍强制交坏稿 + 挂系统脚注」 |
| 证据空 / verify fail / ceiling → **telemetry 与 eval 标签** | 把评测可观测原文镜像进主答复 |
| 出站格式闸拦协议泄漏（如 `DSML`）→ repair / 灾难口 | 解析 provider 私有 tool 协议当产品能力 |

设计与诊断：`docs/engineering/2026-08-10-harness-llm-user-channel-philosophy-diagnosis.md`（§17 方案补丁）。

### Stop decision & agent-lane orchestration (Lead + Workers)

Product **rag / search** (any non-empty `capabilities[]` containing `rag` and/or `search`) target **Lead Agent + specialized Workers**. Design: `docs/plans/2026-08-11-lead-rag-web-workers-design.md`.  
Legacy single-brain SaC union and the 2026-08-07 **three-loop verify path** are being replaced on this lane; product YAML already has `verify: false` (no independent verify LLM).

| Role | Does | Must not |
|------|------|----------|
| **Lead** | 指代消解、拆解 Task Brief、调度、覆盖度裁决、用户 prose 合成 | 直接 dense/web 检索找料（补料只 re-brief Worker）；把 pack/host 标签拼进用户主气泡 |
| **RAG Worker** | 短程 SaC（dense/lexical/grep…）→ `evidence_pack_v1` | 调 web；写用户终答 |
| **Web Worker** | host 检索叶子（可多 query + CRW）→ `evidence_pack_v1` | 调 dense/grep；写用户终答 |
| **Host** | 结构门（Brief/PackGate/`tool_ok_count` 重算）、re-brief≤1、格式出站闸、进度 Delegate | 语义「覆盖够了」拒答句；用户主气泡脚注 |

| Term (prefer) | Meaning | Owner |
|---------------|---------|--------|
| **Task Brief** | Lead→Worker 结构化简报（`task_brief_v1`） | Lead 写；host 启动门 |
| **EvidencePack** | Worker→Lead 证据契约（`evidence_pack_v1`）；无自报 grounding flag | Worker 填；**host PackGate** |
| **re-brief** | 最多 **1** 次补派 Worker | Lead 请求；host 计数 |
| **Continue** | Worker 内下一检索回合 | Worker 模型 / 结构门 |
| **DirectAnswer** | 无检索合成的终答 — **chat / write_refine** 或 Lead 判定 `base_tools`/`none` | 那些路径的模型 |
| **Lead synthesize** | 用户可见交付（prose + 引用） | Lead；无独立 verify 环 |
| **observation** | Host 注入的第三人称事实（`prompts/loop/*`）— **仅模型信道** | Host reports; model acts |
| **require_evidence** | Skill/Lead **意图**：关键事实锚定 observation/pack | Lead/skill prose；host 只数 Ok |
| **evidence_missing_continue** | 结构：应检索却零 Ok → 注入观察并 Continue（Worker 内或派工前） | Host structural |
| **token / round budget** | 成本顶；为 Lead 合成预留；**无**用户可见 host 披露脚注 | Host cost policy |

**No independent verify loop on this path.** Coverage / grounded / whether to hard-answer is **Lead** adjudication at synthesize time (prompt + telemetry). Host does **not** invent multi-entity scanners or soft-refusal keyword bars.

**BASE tools** (`weather_query` / `calculator` / `user_context` / …): owned by **Lead** (or pure chat), not stuffed into `preferred_source: rag|web`. Ok `weather_query` remains enough for live weather statements without pack grounding theater.

**Do not:** golden-set leakage; host semantic completeness veto beyond Ok-count structural gates; Worker user-facing final prose; dual single-brain KB∪web union as the long-term path.

**Product path (W0–W3):** rag / search / dual assemble to `LeadWorkers`. `HostWeb` direct-answer code may remain for tests only — not product assemble.

## Product hard rules (`avrag-rs`) — non-negotiable (formerly §8)

| # | Rule (single-line form; full text: `docs/agent/product-apps.md`) |
|---|------|
| T1 | No new business methods on `AppState` / shallow faces → domain service or target Product App |
| T2 | Write forever outside ReAct ToolCatalog; `write_refine_*` only via `write_refine::tool_specs_for_pool` |
| T3 | Chat/RAG/Search tool execute only via `ToolCatalog` / `dispatch_tool` |
| T4 | No C4: Capability / Skill / Tool stay three layers |
| T5 | Behavior-preserving slices for the **current** contract; verify with targeted `cargo test -p …` / L1 — not a license for dual APIs or compat shims (see Design principles) |
| T6 | Solo local trunk; no CI theater |
| T7 | `workspace` is the sole product truth (never new notebook-primary APIs) |
| T8 | No product `org`: ownership `user_id`/`owner_user_id`, scope `workspace_id` |

- Execute only via `state.conversation().execute` / `execute_stream`. Sessions/search/citations via `state.agent()`. Documents/workspaces via `state.workspace()`.
- Fix failures **toward** `workspace` / `user`, never "align back" to `notebook` / `org`.

## Code intelligence

- Structure / relations / blast radius: **code-review-graph first** (MCP `code-review-graph` server: `get_minimal_context_tool` → `get_impact_radius_tool` / `query_graph_tool` / `detect_changes_tool`, or CLI `code-review-graph build|update|detect-changes`). Full rules: `docs/agent/code-review-graph.md`.
- Semantic chunk search: `semble`; exact literal strings: `grep` last.
- **After structural code changes, run `code-review-graph update` in the same session before claiming done.** Never commit `.code-review-graph/`.

## Environment & solo

- Credentials: always read `avrag-rs/.env` (+ `.env.example`) first; **reuse configured values silently, never re-ask**; persist user-supplied new values to `.env` incrementally.
- Default: local trunk `master`; commit locally. No push / PR / CI babysitting unless the user asks. Full discipline: `docs/engineering/SOLO_DISCIPLINE.md`.
- Services (Milvus/PG/Redis/MinIO): assume running per `docs/agent/wsl-services.md`; do not `docker-compose up` blindly; do not prune `avrag-test-pg-*` containers.
- Deploy: only `scripts/deploy-*.sh` (status: `scripts/deploy-status.sh`). Never ad-hoc ssh/scp product code from chat.
- **VPS fleet: main only.** Cloud product host is a single machine (`VPS_MAIN_*` in `.env` — backend, frontend, public sites). The former **qdrant** VPS subscription is cancelled; do **not** use or reintroduce `VPS_QDRANT_*`, and do not assume a second cloud box for vectors (local/SaaS retrieval is pgvector or Milvus per `RETRIEVAL_BACKEND`, not a dedicated Qdrant host).

## Verify defaults

- Touched Rust: `cargo test -p <pkg> --lib` as relevant (`app-bootstrap`, `app-chat`, `agent-tools`, …); wave end or on request → `bash scripts/test-l1.sh`.
- Frontend (`frontend_next`): `pnpm test` / typecheck.
- Frontend style baseline is mechanically enforced by `frontend_next/tests/style/design-baseline.test.ts`: no numeric font-weight ≥ 500 (tokens are all 400), no bare hex outside token files, no drop shadows except allowlisted floating overlays. Nav destinations live only in `frontend_next/lib/navigation/nav-config.ts` (PRODUCT_IA §4); route existence guarded by `tests/navigation/nav-config.test.ts`.
- WSL: respect `jobs=2`; never stack concurrent full `cargo test` runs. Details: `docs/agent/rust-resources.md`.
- Real LLM / full Playwright: not required mid-wave (E2E semantics: `avrag-rs/docs/e2e-gates.md`).
- Long E2E runs (full-149, staging ingest, DR2/L3): never block on one global timeout — background + log file + poll progress; use `scripts/with-watchdog.sh` (silence kill) and `scripts/test-full149.sh` (canonical full-149, circuit breaker `E2E_ABORT_AFTER_CONSECUTIVE_FAILS=8`). Full conventions: `avrag-rs/docs/e2e-gates.md` §Agent run conventions.

## Repo map

- Root WSL: `/home/chuan/context-osv6` · Windows: `Z:\home\chuan\context-osv6`
- Product backend: `avrag-rs` · Frontend: `frontend_next` (Next.js + React + TS, pnpm) · `frontend_rust` workspace only when explicitly asked.

## More (links only)

- `docs/README.md` — 文档索引：哪些是现行权威、哪些已被取代、ADR 编号已知问题
- `docs/agent/product-apps.md` — full T1–T8 / workspace / org text
- `avrag-rs/prompts/README.md` · `avrag-rs/prompts/loop/README.md` — prompt CDS + loop nudge load path
- `docs/agent/code-review-graph.md` — code-review-graph query & update rules
- `docs/agent/wsl-services.md` — services, ports, VPS
- `docs/agent/rust-resources.md` — target/cache policy
- `docs/agent/coding-behavior.md` — original long-form behavior essays (human reference)
- `docs/engineering/SOLO_DISCIPLINE.md` · `docs/adr/0007-product-apps-composition-root.md`
