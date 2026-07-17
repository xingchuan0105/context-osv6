# Orchestrator + Channel Workers + Chat Exit — Implementation Plan

> **For agentic workers:** Use subagent-driven-development or executing-plans. Track steps with `- [ ]`.

**Goal:** Replace the agent-lane **unioned single ReAct** (`mode_assemble` tool/skill merge) with **Orchestrator agent loop (delegate-only) → Rag/Search workers (EvidencePack only) → Chat agent (sole final answer)**. Preserve product `capabilities[]` (RAG/Search tags only). Close dual “web-only / 未提供报告” via §7 materialize + completion invariant + synthesize policy.

**Design:** [`ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md`](./ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md)  
**Related:** capabilities multiselect design/plan (2026-07-15); unified synthesis contract.

**Architecture (locked):**

```text
ConversationApp.execute*
  → preflight / CapabilitySet (unchanged product gate)
  → if flag off: legacy mode_assemble union path
  → if flag on: OrchestratorHost
        materialize workers from caps (§7.1)
        Orchestrator ReAct (delegate_rag | delegate_search | delegate_chat only)
        Workers: short channel ReAct → EvidencePack
        Chat agent: direct | synthesize (Option B)
```

**Tech stack:** Rust `app-chat`, `agent-loop`, `agent-tools`, `app-bootstrap`; existing `ReActLoop` / mode YAML as worker configs; feature flag env; no new frontend capability tags.

**Solo verify:** targeted `cargo test -p app-chat --lib`, `cargo test -p agent-loop --lib`; no CI theater; no full Playwright unless asked.

**Iron rules:** no new AppState business methods; Write lane untouched; workspace term not notebook.

---

## Feature flag

| Env | Default | Behavior |
|-----|---------|----------|
| `AGENT_ORCHESTRATOR_V1=1` | **off** until O1 green | New host path |
| unset / `0` | on today | Legacy `mode_assemble` + union ReAct |

Document in `avrag-rs/.env.example`. Rollback = flag off.

---

## File map (planned)

### New modules

| Path | Role |
|------|------|
| `avrag-rs/crates/app-chat/src/orchestrator/mod.rs` | Host entry, flag, turn run |
| `avrag-rs/crates/app-chat/src/orchestrator/types.rs` | `TaskBrief`, `EvidencePack`, `DispatchRecord`, `ChatHandoff` |
| `avrag-rs/crates/app-chat/src/orchestrator/materialize.rs` | §7.1 caps → channel set |
| `avrag-rs/crates/app-chat/src/orchestrator/invariant.rs` | §7.2 completion checks + default brief |
| `avrag-rs/crates/app-chat/src/orchestrator/delegate.rs` | Run Rag/Search/Chat handoffs |
| `avrag-rs/crates/app-chat/src/orchestrator/workers.rs` | Thin wrappers: load rag/search ModeConfig, run loop, map → EvidencePack |
| `avrag-rs/crates/app-chat/src/orchestrator/chat_exit.rs` | Chat direct / synthesize from packs |
| `avrag-rs/modes/orchestrator.yaml` **(create)** | Orchestrator mode: tool_pool = delegate tools only; budget |
| `avrag-rs/prompts/orchestrators/orchestrator-base.md` **(create)** | Minimal orchestrator facts (paradigm + brief rules) |
| `avrag-rs/prompts/orchestrators/chat-exit.md` **(create)** or reuse chat synthesis | Chat direct/synthesize + §7.3 policy bullets — **deviation accepted (2026-07-17):** inlined in `orchestrator/chat_exit.rs` (`render_synthesize_context`); single source stays in Rust, unit-tested. No md file created. |

### Modify

| Path | Role |
|------|------|
| `avrag-rs/crates/app-chat/src/lib.rs` | `mod orchestrator` |
| `avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs` | Branch flag → OrchestratorHost vs legacy assemble |
| `avrag-rs/crates/app-chat/src/agents/unified/mod.rs` | Optional: call host instead of single ModeConfig when metadata says so |
| `avrag-rs/crates/agent-tools/...` | Register `delegate_rag` / `delegate_search` / `delegate_chat` **or** host-side tool dispatch without full SkillRegistry (prefer **host intercept** of tool names to avoid fake tools in global catalog for non-orchestrator modes) |
| `avrag-rs/crates/agent-loop/.../exit_policy.rs` | Do **not** rely on for dual; workers keep own evidence; orchestrator uses invariant module |
| Progress / SSE mapping | Map dispatch phases for UI |
| `avrag-rs/.env.example` | Flag + short comment |
| Design § already updated | — |

**Prefer host-side intercept** of orchestrator tool calls over registering global skills that other modes could see.

---

## Waves → tasks

| Wave | Outcome |
|------|---------|
| **O1** | Flag path works; materialize + invariant; B exit; dual cannot skip rag dispatch |
| **O2** | Multi-hop quality; progress; pack limits; richer notices |
| **O3** | Remove/retire union assemble from product path; docs/ADR note |

---

### Task 1: Types + materialize + invariant (no LLM)

**Files:**
- Create: `orchestrator/types.rs`, `materialize.rs`, `invariant.rs`, `mod.rs`
- Test: unit tests in same modules

**Steps:**

- [x] Define `Channel { Rag, Search }`, `TaskBrief { goal: String, ... }`, `EvidencePack`, `DispatchRecord`, `ChatHandoff { mode: Direct|Synthesize, packs, instruction, partial_notices }`.
- [x] `materialize(caps: CapabilitySet) -> Vec<Channel>` — empty caps → empty vec.
- [x] `assert_complete(materialized, records) -> Result<(), MissingChannel>`; `default_brief(channel, user_query) -> TaskBrief` for recovery.
- [x] Unit tests: `[]` → no channels; `[rag]` → rag only; dual → both; invariant fails if only search records for dual.

**Verify:** `cargo test -p app-chat --lib orchestrator -- --nocapture`

---

### Task 2: Workers → EvidencePack (wrap existing loops)

**Files:**
- Create: `orchestrator/workers.rs`
- Reuse: `load_mode_config("rag"|"search")`, existing agent run entry used by unified agent

**Steps:**

- [x] `pack_from_run` / channel mapping (rag chunks + search results) → EvidencePack  
- [x] Cap `items` length (top 12) and text size per item.
- [x] Unit tests for extract/empty/error.

**Verify:** `cargo test -p app-chat --lib orchestrator::workers`

---

### Task 3: Chat exit (Option B)

**Files:**
- Create: `orchestrator/chat_exit.rs`
- Prompt: `chat-exit.md` or inject §7.3 bullets into existing chat/synthesis assembly

**Steps:**

- [x] `direct_handoff` / `synthesize_handoff` + `query_for_agent` (packs JSON + §7.3 notices).
- [x] §7.3 partial_notices + banned-copy helper (`looks_like_user_did_not_provide_doc`).
- [x] Tests for empty rag notices.

**Verify:** `cargo test -p app-chat --lib orchestrator::chat_exit`

---

### Task 4: Orchestrator mode + delegate host

**Files:**
- Create: `modes/orchestrator.yaml`, `prompts/orchestrators/orchestrator-base.md`
- Create: `orchestrator/delegate.rs`, host `run_orchestrated_turn`

**Steps:**

- [x] `modes/orchestrator.yaml` + `orchestrator-base.md` (for O2 LLM controller).
- [x] O1 host: **structural first-wave** all materialized channels via `default_brief` + `OrchestratorExecutor` (no LLM skip).
- [x] §7.2 recovery path if records missing; then chat synthesize (B).
- [x] `AgentServiceExecutor` production path (single-channel assemble + chat exit).
- [x] Mock host tests: pure chat / dual both channels / rag-only.

**Verify:** `cargo test -p app-chat --lib orchestrator`

---

### Task 5: Pipeline wire + flag

**Files:**
- Modify: `pipeline_steps.rs` / unified dispatch
- Modify: `.env.example`

**Steps:**

- [x] `dispatch_agent_mode` branches on `orchestrator_v1_enabled()`.
- [x] turn_metadata: orchestrator + dispatches + pack_statuses.
- [x] Activity stages: `delegate_rag` / `delegate_search` / `compose:compose_answer`.
- [x] Flag default off; `.env.example` comment.

**Verify:** full `cargo test -p app-chat --lib` (110 ok).

---

### Task 6: Dual regression (incident class)

**Files:**
- Test under `app-chat` orchestrator tests (mock LLM sequence)

**Scenario:**

1. caps = rag+search — covered by `host::tests::dual_runs_both_channels`  
2. Assert both channels dispatched; rag empty + search ok → partial notices; answer not banned user-blame copy  

**Verify:** `cargo test -p app-chat --lib orchestrator::host`

---

### Task 7: O2 — multi-hop + observability (after O1 green)

- [ ] Orchestrator prompt examples: serial RAG then Search; gap re-search.  
- [ ] Progress SSE: `delegate_rag`, `delegate_search`, `compose` (chat).  
- [ ] Pack token budget hard cap shared into chat context.  
- [ ] Optional second worker round metrics.

**Verify:** targeted tests + one manual dual compare query with flag on.

---

### Task 8: O3 — retire product union path

- [ ] When flag default on and stable: remove or dead-code `mode_assemble` union from product execute (keep loaders for workers).  
- [ ] ADR note or design status “Implemented”.  
- [ ] `graphify update .` if structure landed.  
- [ ] Do **not** delete write-core.

**Verify:** `cargo test -p app-chat --lib`; `cargo test -p agent-loop --lib` as needed.

---

## Explicit non-goals (plan)

- Frontend new tags for orchestrator/chat.  
- Subagent final answers (Option A).  
- Global ToolCatalog pollution with delegate_* for all modes.  
- Full rewrite of progressive disclosure framework.  
- VPS deploy / PR / CI expansion.

---

## Risk register

| Risk | Mitigation |
|------|------------|
| Latency +1 synthesize | Accept (B); stream chat; short synthesize system |
| Host reimplements ReAct poorly | Reuse complete_with_tools + small loop; copy patterns from ReActLoop |
| Workers still too fat | Same mode YAML first; slim later |
| Flag on breaks e2e | Default off; turn on after Task 6 green |
| Orchestrator ignores brief quality | O2 prompt; O1 still has default_brief recovery |

---

## Definition of done (O1)

- [ ] Flag off: legacy behavior.  
- [ ] Flag on: caps=[] → chat direct only; no workers.  
- [ ] Flag on: caps include rag → ≥1 rag dispatch record before answer.  
- [ ] Flag on: dual → both channels in materialize; invariant holds.  
- [ ] Final answer always from chat_exit (B).  
- [ ] Design §7 wording reflected in code comments / module docs.  
- [ ] Targeted unit tests pass; no requirement for full product e2e this wave.

---

## Suggested implementation order

1 → 2 → 3 → 4 → 5 → 6 → (ship flag default off) → 7 → 8.

---

## Doc links

| Doc | Role |
|-----|------|
| [ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md](./ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md) | Spec |
| [CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md](./CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md) | Product caps (unchanged surface) |
| [UNIFIED_SYNTHESIS_CONTRACT_2026-07-15.md](./UNIFIED_SYNTHESIS_CONTRACT_2026-07-15.md) | Chat synthesize envelope |
