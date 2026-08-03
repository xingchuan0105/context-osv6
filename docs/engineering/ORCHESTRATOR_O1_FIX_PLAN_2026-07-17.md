# Orchestrator O1 Fix Plan — Citations, Progress i18n, Worker Hygiene

> **SUPERSEDED** — 本文描述的 orchestrator / worker 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

> **For agentic workers:** Track steps with `- [ ]`. Targeted verifies per task; no CI theater.

**Goal:** Close the gaps/bugs/drift found in the 2026-07-17 review of the O1 orchestrator implementation before `AGENT_ORCHESTRATOR_V1` can be turned on. Scope: **P0** (citations lost, `rag+search` progress restore), **P1** (progress i18n, O2 decision), **P2** (retrieval-query pollution, double-synthesis cost), **D4** (plan bookkeeping, dead-code annotation), plus commit hygiene.

**Review basis:** `ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md` / `..._PLAN_2026-07-16.md`; code under `avrag-rs/crates/app-chat/src/orchestrator/`.

**Current state:** O1 implemented behind flag (default off); `cargo test -p app-chat --lib` 110/110 green; legacy union path untouched when flag off.

---

## Bug/drift register (from review)

| ID | Sev | Item |
|----|-----|------|
| B1 | P0 | Chat exit runs pure-chat `prose_only` → no `[[cite:]]`/`[[web:n]]` contract, prompts forbid markers → **citations/sources lost** in orchestrator path (design §4.3 unimplemented) |
| B3 | P0 | `progressSnapshotFromTurnMetadata` mode whitelist lacks `"rag+search"` → dual-turn progress panel lost after reload (legacy union path also affected) |
| B2 | P1 | Host emits raw English Activity messages (`"dispatch rag"`, `"chat synthesize exit"`, `"direct chat exit"`); frontend only localizes `progress.*` keys → English leak in zh UI; stage names break `{phase}:{kind}` convention |
| O2 | P1 | Design header says "O1 implemented" but no LLM orchestrator exists; `modes/orchestrator.yaml` + `orchestrator-base.md` are zero-reference files; need explicit decision |
| D3 | P2 | `run_channel` overwrites `req.query` with English brief goal (`host.rs:165`) → rag `inject_retrieval_query` retrieves on wrapper text, polluting recall |
| D2 | P2 | Workers run full ReAct + full synthesis, answer truncated to 800-char notes; chat exit synthesizes again → double LLM synthesis per channel per turn |
| D4 | D4 | Plan file-map listed `prompts/orchestrators/chat-exit.md` (never created — inlined in `chat_exit.rs`, acceptable); §7.2 recovery branch (`host.rs:114-124`) unreachable today (first wave always records); both need bookkeeping, not code |

---

## Wave F0 — P0: citations + progress restore

### Task F0.1: B1 — citations through the chat exit (Option B complete)

**Root cause:** `AgentServiceExecutor::run_chat` assembles `CapabilitySet::default()` (pure chat) → `ProseOnly` contract → no marker instructions; `render_synthesize_context` never mentions markers; worker `tool_results` are discarded after pack building, so nothing can rebuild citations downstream.

**Design (reuse existing machinery — no new contract):**

1. Chat exit prompt instructs markers (in `render_synthesize_context`, `chat_exit.rs`):
   - Doc facts: `[[cite:ID]]` where ID = rag pack item `id` (chunk_id, copied verbatim).
   - Web facts: `[[web:n]]` where n = 1-based index of the item in the search pack `items` array (matches `citation_index` from `web_search`).
   - Keep §7.3 partial-notice rules; do not emit markers for channels whose pack is empty/error.
2. Host retains worker run output: `run_orchestrated_turn` collects each channel run's `tool_results` (currently dropped in `host.rs:105-110`), merged across channels (and across §7.2 recovery runs).
3. After `run_chat`, rebuild evidence on the final `AgentRunResult` (new helper, e.g. `workers.rs::attach_evidence`):
   - `answer_result.tool_results` = merged worker tool_results (+ keep chat's own, e.g. `user_context`);
   - `answer_result.citations` = `agent_loop::helpers::citations::filter_citations_for_mode("rag", &answer, build_all_citations_from_tool_results(&merged))` — `"rag"` mode id keeps both doc cites and web indices (see `helpers/citations.rs:156-185`);
   - `answer_result.sources` = `agent_loop::helpers::retrieval::build_sources_from_tool_results(&merged)`.
   - No SSE work needed: `pipeline_steps.rs:686-697` (`emit_terminal_stream_events`) already emits the `Citations` event when `response.citations` is non-empty and none were streamed; `build_chat_execution_from_result` (`service_modes.rs:99-100`) maps `agent_result.citations/sources` as-is.

**Files:**
- Modify: `orchestrator/chat_exit.rs` (marker instructions in synthesize context)
- Modify: `orchestrator/host.rs` (retain worker `tool_results`; attach evidence post-chat)
- Modify: `orchestrator/workers.rs` (helper to rebuild citations/sources; re-export in `mod.rs`)

**Steps:**
- [x] Add citation-marker section to `render_synthesize_context` (doc `[[cite:id]]`, web `[[web:n]]` index rule, only for non-empty packs).
- [x] `OrchestratedTurn` / host: carry merged worker `tool_results`; after `run_chat`, rebuild `citations` + `sources` on `answer_result` via `agent_loop::helpers`.
- [x] Unit test (mock executor): worker run returns rag chunks + web results; chat answer contains `[[cite:chunk-a]]` + `[[web:1]]` → `turn.answer_result.citations` contains both, correct kinds/ids; no markers → empty.
- [x] Unit test: empty rag pack → synthesize context contains no `[[cite:]]` instruction for rag (and keeps 未命中 notice).

**Verify:** `cargo test -p app-chat --lib orchestrator` ✅ (22 passed; dedup added for multi-dispatch citation repeats)

---

### Task F0.2: B3 — `rag+search` progress snapshot restore

**Files:**
- Modify: `frontend_next/hooks/chat-session/progress-i18n.ts:102` (add `"rag+search"` to the mode whitelist)
- Modify: `frontend_next/tests/workspace/progress-i18n.test.ts` (dual-mode restore case)

**Steps:**
- [x] Accept `"rag+search"` in `progressSnapshotFromTurnMetadata` (type already allows it via `WorkspaceChatMode`, `ui-store.ts:11`).
- [x] Test: turn_metadata with `progress.mode = "rag+search"` restores activities instead of returning null.

**Verify:** `cd frontend_next && pnpm vitest run tests/workspace/progress-i18n.test.ts` ✅ (5 passed)

---

## Wave F1 — P1: progress i18n + O2 decision

### Task F1.1: B2 — orchestrator activities as localized WorkFacts

**Design:** stop emitting raw `AgentEvent::Activity` strings from the host; route through `agent_loop::progress::emit_work_fact` so titles are stable `progress.*` keys (frontend `progress-i18n.ts` localizes keys, falls back to raw text only for legacy).

**Files:**
- Modify: `avrag-rs/crates/agent-loop/src/progress/mod.rs` (new `ProgressKind::DelegateRag` / `DelegateSearch` → keys `progress.delegate_rag` / `progress.delegate_search`, phase `Act`; stage becomes `act:delegate_rag` / `act:delegate_search`)
- Modify: `orchestrator/host.rs` (replace raw emissions: delegates → new facts; compose → existing `WorkFact::compose_answer()`; direct chat → existing `WorkFact::understand(&query)`)
- Modify: `frontend_next/lib/i18n/messages/workspace.ts` (`progress.delegate_rag`, `progress.delegate_search`, zh-CN + en)
- Tests: agent-loop progress test for new kinds; frontend i18n parity if a test enumerates keys

**Steps:**
- [x] Add the two `ProgressKind` variants + `as_str` mappings + unit test (`agent-loop/src/progress/mod.rs` tests).
- [x] Host: `emit_work_fact` for delegate (detail = brief goal, truncated), compose, understand; delete raw `AgentEvent::Activity` emissions.
- [x] Frontend keys both locales.
- [x] Check `assistant_progress_turn_metadata` snapshot still records the new facts (records all Activity events — `sse_sink.rs:116-126`, no stage whitelist).

**Verify:** `cargo test -p agent-loop --lib progress` ✅; `cargo test -p app-chat --lib orchestrator` ✅; `pnpm vitest run tests/workspace/progress-i18n.test.ts` ✅

---

### Task F1.2: O2 decision — record it, don't drift

**Recommendation (default unless overridden):** keep the **structural host** as the O1 runtime; defer the LLM orchestrator loop until F0+F1 land **and** one real flag-on dual query validates quality/latency. The structural first wave already closes the incident class (§7.1/§7.2 by construction); the LLM controller adds multi-hop quality, not correctness.

**Steps:**
- [x] Amend `ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md` header: O1 = structural host (no LLM orchestrator, no `delegate_*` tool surface); LLM controller remains O2. **(Decision: defer O2 per recommendation.)**
- [x] Add header comment to `modes/orchestrator.yaml` + `prompts/orchestrators/orchestrator-base.md`: "O2 artifact — not loaded by runtime (kept for O2)". (yaml comment added; the md already carries the O2 note in its closing line.)
- [x] Decision recorded in this plan's log below; if decision = "drop O2 entirely", delete both files instead and strip §6 multi-hop from design.

**Verify:** docs only.

---

## Wave F2 — P2: worker query hygiene + synthesis cost

### Task F2.1: D3 — keep user query clean; brief goes to prompt parts

**Files:**
- Modify: `orchestrator/host.rs` (`AgentServiceExecutor::run_channel`)

**Steps:**
- [x] Do **not** overwrite `req.query` with `brief.goal`; keep the original user query (so `inject_retrieval_query` and codegen retrieve on the user's words).
- [x] Append the brief to the worker's `system_prompt_parts` instead: after `assemble_mode`, push `"## Task brief (orchestrator)\n{goal}"` onto `assembled.system_prompt_parts` before inserting metadata.
- [x] Unit test: captured worker request `query` == original user query; `system_prompt_parts` contains the brief.

**Verify:** `cargo test -p app-chat --lib orchestrator` ✅ (`worker_keeps_user_query_brief_goes_to_prompt_parts`)

---

### Task F2.2: D2 — slim worker output (measure first, then cut)

**Steps:**
- [ ] Measure: run one flag-on dual query locally (real LLM); record wall time + per-stage LLM calls from usage observer / debug payload. Baseline = legacy union path same query. **(manual step — needs running stack)**
- [x] Slim: worker system parts get a "machine notes" override — worker synthesis produces concise evidence bullets for the orchestrator (no user-facing prose). Implemented as an appended system-prompt part in `run_channel` (prompt-level; no loop surgery). Contract stays per mode YAML.
- [ ] Re-measure same query; expect synthesis tokens per worker to drop materially. If still heavy, consider worker loop config override (e.g. lower `max_iterations` for workers) as a follow-up — do **not** fork mode YAMLs in this wave. **(blocked on measurement)**

**Verify:** manual run (pending) + `cargo test -p app-chat --lib orchestrator` ✅ (notes-override covered by `worker_keeps_user_query_brief_goes_to_prompt_parts`).

---

## Wave F3 — D4 bookkeeping + hygiene

### Task F3.1: plan/code bookkeeping

**Steps:**
- [x] `ORCHESTRATOR_SUBAGENT_CHAT_PLAN_2026-07-16.md` file map: `chat-exit.md` → "inlined in `orchestrator/chat_exit.rs` (deviation accepted; single source stays in Rust, tested)".
- [x] `host.rs` §7.2 recovery branch: comment that first wave guarantees records; branch is defense for the future O2 LLM-dispatch path where a channel may be skipped. No code change.
- [x] Design doc §12.1: add test-scenario row "orchestrator synthesize emits `[[cite:]]`/`[[web:n]]` → response.citations non-empty" (covered by F0.1 test).

### Task F3.2: commit hygiene + graph

**Steps:**
- [ ] Split the current working tree into focused commits: (a) orchestrator implementation + tests; (b) prompt/mode/mode_assemble changes; (c) docs; (d) unrelated cleanups (`.serena` removal, `site-map.ts`, `next-env.d.ts`, `STYLE_BASELINE.md`, writing-style-mcp plans) each separate. **(pending user go-ahead for git commits)**
- [x] After F0–F2 land: `cd /home/chuan/context-osv6 && graphify update .` (new `orchestrator/` module + progress kinds alter structure). ✅ 89636 nodes; `attach_worker_evidence` confirmed indexed.

**Also fixed while verifying:** pre-existing committed failure `chat_mode_config_has_empty_retrieve_tool_pool` (chat.yaml gained `tool_pool: [user_context]` in a4ed59c but the test still asserted empty) → updated to `chat_mode_config_has_only_user_context_tool` matching the D9 base-tool design.

---

## End-to-end acceptance (after F0 + F1)

- [ ] Flag on, `caps=[rag,search]`, doc in scope, compare-style query (real LLM, local): both `delegate_*` activities render localized; answer carries working citation chips (doc + web); no "未提供报告" copy; reload page → progress panel restores (F0.2).
- [ ] Flag on, `caps=[]`: pure chat unchanged, no worker activity.
- [ ] Flag off: legacy union behavior bit-identical (no regressions in `cargo test -p app-chat --lib`, 110+ tests).

## Explicit non-goals (this plan)

- O2 LLM orchestrator implementation (decision only, F1.2).
- Multi-hop re-dispatch, pack token budgets, fan-out/fan-in (O2).
- Frontend capability-tag changes (none needed; surface unchanged).
- Write lane / write-core changes.
- CI expansion, VPS deploy.

## Definition of done

- [x] F0.1 + F0.2 merged: citations work end-to-end under flag; dual progress restores. (unit/integration level; real-LLM e2e below still pending)
- [x] F1.1: no raw English activity strings from orchestrator host; keys in both locales.
- [x] F1.2: O2 decision recorded (defer LLM controller); design header accurate.
- [x] F2: worker query unpolluted; worker notes slimmed; **dual latency measurement still pending (manual, needs running stack)**.
- [x] F3: plan/doc comment updates; graphify updated; **commit split pending user go-ahead**.
- [x] Targeted tests green: `cargo test -p app-chat --lib` (115), `cargo test -p agent-loop --lib` (180), frontend progress tests (5).
