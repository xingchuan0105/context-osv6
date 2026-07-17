# Design: Orchestrator Agent + Channel Subagents + Chat Exit

**Date:** 2026-07-16  
**Status:** Approved (product / architecture design); **O1 implemented** (flag `AGENT_ORCHESTRATOR_V1`, default off) as a **structural host** — code-driven first-wave dispatch + chat exit (Option B). The LLM orchestrator loop, `delegate_*` tool surface, and multi-hop re-dispatch (§4.1, §6) are **not** part of O1; they remain O2, deferred until O1 fixes land and one real flag-on dual query validates quality/latency (fix plan: [ORCHESTRATOR_O1_FIX_PLAN_2026-07-17.md](./ORCHESTRATOR_O1_FIX_PLAN_2026-07-17.md)).  
**Supersedes (runtime shape):** Single ReAct brain with capability **tool/prompt union** (`mode_assemble` dual path) for agent-lane execute.  
**Builds on:** [CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md](./CAPABILITIES_MULTISELECT_AND_USER_CONTEXT_DESIGN_2026-07-15.md), [UNIFIED_SYNTHESIS_CONTRACT_2026-07-15.md](./UNIFIED_SYNTHESIS_CONTRACT_2026-07-15.md)  
**Scope:** Agent-lane orchestration after product `capabilities[]` is resolved; not Write lane; not new frontend capability tags.

---

## 1. Goals and non-goals

### Goals

1. Fix dual-capability failure mode: one ReAct agent **prefers easy web tools**, skips workspace retrieval, then claims “user did not provide the report.”
2. Split **scheduling**, **channel execution**, and **user-facing answer** into three roles with hard interfaces.
3. Keep product surface: multi-select **RAG** / **Search** only; empty = pure chat. No new user-visible capability for “chat” or “orchestrator.”
4. Support multi-hop research (e.g. doc vs best-practice): orchestrator assigns **targeted** sub-tasks, may re-dispatch after seeing evidence.
5. **All final answers** produced by a single **Chat agent** (direct or grounded synthesis) — **Option B** (locked).

### Non-goals (this design)

- Exposing orchestrator / chat / subagents as frontend tags or mode pickers.
- Re-introducing Write as product capability.
- Full multi-agent framework productization (Crew/AutoGen-style UX).
- Deleting write-core or legacy mode YAML files in this document wave.
- Replacing Product Apps / ConversationApp as the transport entry (still `conversation().execute*`).

---

## 2. Problem statement (why change)

Current agent lane (post capability multiselect):

```text
One ReAct loop + agent-base + optional manuals
  + tool_pool / skill union
  + shared iteration budget
```

Observed (workspace dual RAG+Search, doc in scope):

| Fact | Implication |
|------|-------------|
| `capabilities: ["rag","search"]` and `doc_scope` present | Product gate worked |
| Tools used: only `user_context` + `web_search` | Channel isolation failed inside one brain |
| Progress: web only → budget_exhausted → compose | Orchestration and execution mixed |
| Answer: “您未提供具体的转型报告内容” + only `[[web:n]]` | Model never saw document evidence |

Structural causes:

1. **Asymmetric tools:** web tools are native and easy; RAG is codegen-heavy.
2. **Unioned context:** one system prompt / one budget for incompatible protocols.
3. **Evidence gate** not dual-aware (`mode.id = "rag+search"` fell through to “always has evidence”).
4. **Prompt-only** fixes cannot force “must retrieve docs when rag is selected.”

---

## 3. Locked decisions

| # | Decision | Choice |
|---|----------|--------|
| D1 | Final answer ownership | **Option B:** Subagents **never** write user-facing final answers. **Chat agent** always does (direct or synthesize). |
| D2 | Orchestrator role | **Task allocation only** — no channel retrieval, no final prose. |
| D3 | Orchestrator control style | **Agent loop (ReAct)** over **delegate-only** tools (not one-shot plan-only, not pure hard-coded query copy). |
| D4 | Orchestrator paradigm | Decides **execution topology**: direct, serial, parallel, hierarchical / multi-hop re-dispatch, fan-out/fan-in. |
| D5 | Frontend capabilities | Still only **RAG** / **Search** (or empty). Chat and orchestrator are **internal**. |
| D6 | Selected channels | **Three orthogonal layers** (not one “force” slogan) — see §7: (1) **channel materialization**, (2) **completion invariant**, (3) **synthesize policy**. |
| D7 | Dual partial packs | Covered by §7.3 synthesize policy: answer from available packs + **notice**; empty ≠ “user never provided text.” |
| D8 | Pure chat (`capabilities=[]`) | Graph has **no** channel workers; Orchestrator → Chat `direct` only. |
| D9 | `user_context` | Remains **base skill/tool** (catalog Use when); not a capability tag; prefer chat-side availability; **not** long system essays. |

---

## 4. Role model

```text
                    ChatRequest
                    capabilities[], query, doc_scope, …
                           │
                           ▼
              ┌────────────────────────────┐
              │  Orchestrator Agent        │
              │  (ReAct loop)              │
              │  • topology / paradigm     │
              │  • task_brief per dispatch │
              │  • multi-hop re-dispatch   │
              │  • when to call chat       │
              │  DOES NOT retrieve/write   │
              └────────────┬───────────────┘
        delegate_*         │
     ┌───────────┬─────────┴──────────┐
     ▼           ▼                    ▼
 RagWorker   SearchWorker      Chat Agent
 (retrieve)  (retrieve)        (ONLY user-facing
  EvidencePack  EvidencePack    answer: direct |
                                synthesize)
```

### 4.1 Orchestrator Agent

**Is:**

- Scheduler of **paradigm and briefs** on an already-materialized channel set (see §7.1): serial / parallel / multi-hop re-dispatch, what each `task_brief` says.
- Multi-hop controller: after observations, may **re-dispatch** with refined briefs (e.g. gap keywords from RAG pack).
- Chooses when to call Chat (`direct` vs `synthesize` + instruction).

**Is not:**

- Executor of `web_search`, codegen, dense retrieval.
- Author of the final user message.
- Authority to **delete** a selected channel from the turn graph (caps decide materialization; LLM does not un-select RAG/Search).

**Tool surface (conceptual):**

| Tool | Purpose |
|------|---------|
| `delegate_rag` | Run RagWorker with required `task_brief` (+ scope from request) |
| `delegate_search` | Run SearchWorker with required `task_brief` |
| `delegate_chat` | Hand off to Chat agent: `mode=direct \| synthesize` + packs + instructions |
| optional `user_context` | Only if product keeps clock/geo at orchestrator; prefer chat-side if simpler |

No other product retrieval tools on the orchestrator.

### 4.2 Channel Subagents (Workers)

| Worker | Channel | Tools / protocol | Output |
|--------|---------|------------------|--------|
| RagWorker | Workspace docs | Codegen / dense / lexical as today’s RAG mode subset | `EvidencePack` (`channel=rag`) |
| SearchWorker | Web | `web_search` / `web_fetch` | `EvidencePack` (`channel=search`) |

**Rules:**

- Execute **only** the `task_brief` (goal, scope, desired extract shape).
- May use a **short** inner ReAct loop for retrieval refinement within the channel.
- Must **not** call the other channel’s tools.
- Must **not** produce the product final answer (no long user prose as the turn result).
- Optional short **machine notes** (bullet summary for orchestrator) allowed; not shown as the UI answer.

### 4.3 Chat Agent (internal exit)

**Not a frontend capability.** Always available as the sole **user-visible** language exit.

| Mode | When | Input |
|------|------|--------|
| `direct` | `capabilities=[]` or orchestrator chooses pure dialogue | query + history (+ optional user_context) |
| `synthesize` | After ≥1 worker packs (or forced empty packs) | query + history + `EvidencePack[]` + orchestrator instruction |

Chat agent owns:

- Tone / structure / comparison writing.
- Unified citation markers (`[[cite:…]]`, `[[web:n]]` / product-normalized web cites).
- Partial-notice copy when one dual pack is empty/error.
- Refusal / insufficient coverage grounded in packs, **not** “user didn’t paste the document” when RAG was run and doc_scope was set.

---

## 5. Option B (final answer policy) — detail

### 5.1 Why B

| Criterion | Sub writes final (A) | Chat always final (B) |
|-----------|----------------------|------------------------|
| Single writing policy | Split across rag/search/chat skills | One exit |
| Dual vs single path | Divergent | Same synthesize path |
| §7 synthesize policy (partial / 未命中) | Replicated per sub | One place (Chat) |
| Orchestrator purity | Broken if sub “finishes” the turn | Intact |
| Cost | One fewer LLM on single-cap | +1 synthesize LLM on single-cap |

**Locked: B.** Cost mitigated later by short synthesize prompts / streaming, not by giving workers final-answer rights.

### 5.2 Single-capability path (still B)

```text
caps=[rag] → Orchestrator → delegate_rag(brief) → packs
           → delegate_chat(synthesize, packs)
caps=[search] → same with SearchWorker
```

Workers do **not** short-circuit to UI text.

### 5.3 Empty capabilities

```text
caps=[] → Orchestrator → delegate_chat(direct)
```

No Rag/Search workers.

---

## 6. Execution paradigms (orchestrator responsibility)

Orchestrator chooses topology; runtime may expose these as plan structures or as emergent tool-call order.

| Paradigm | Description | Example |
|----------|-------------|---------|
| **Direct** | Only chat | Pure chat |
| **Serial** | Worker A then B using A’s notes in B’s brief | RAG structure → Search best practices |
| **Parallel** | Independent dispatches same wave | Two unrelated facets |
| **Hierarchical / multi-hop** | After packs, re-dispatch | Gap fill Search with doc-specific keywords |
| **Fan-out / fan-in** | Multiple briefs same channel → merge packs → chat | Multi-section doc |

**Typical dual scenario (doc vs best practice):**

1. `delegate_rag`: summarize structure, modules, tech choices, distinctive claims.  
2. `delegate_search`: industry best-practice frameworks for comparison dimensions.  
3. Orchestrator compares pack notes; if gaps → `delegate_search` (or rag) with **targeted** keywords.  
4. `delegate_chat(synthesize)` with all packs + “逐项对比” instruction.  

One-shot static plan cannot do step 3 reliably; **orchestrator ReAct loop is required** for that flexibility. Hard rules alone (always `web(query)` + `dense(query)` once) leave workers without a useful brief.

---

## 7. Channel integrity (three orthogonal layers)

These are **not** three copies of “force the model to search.” They fix different failure modes.

| Layer | Question it answers | Soft (prompt) vs hard (code) |
|-------|---------------------|------------------------------|
| **§7.1 Materialization** | Which channels **exist** this turn? | **Hard:** derived from `capabilities[]` |
| **§7.2 Completion invariant** | Did each materialized channel **run at least once** before Chat synthesize? | **Hard:** assert on dispatch log |
| **§7.3 Synthesize policy** | How to write when packs are partial / empty? | Chat skill + light post-checks; not a second dispatcher |

Orchestrator LLM optimizes **briefs and topology**, not “whether RAG is on.”

### 7.1 Channel materialization (structure)

On each turn, after `CapabilitySet` resolve:

```text
caps=[]           → workers: none; only Chat(direct)
caps⊇{rag}        → graph includes RagWorker (node cannot be removed by LLM)
caps⊇{search}     → graph includes SearchWorker
caps⊇{rag,search} → both nodes present; orchestrator chooses serial/parallel/multi-hop among them
```

**Implications:**

- Selected capability ⇒ channel is **in the graph**, not “optional tool the orchestrator might ignore.”
- O1 preferred implementation: **code enqueues a first dispatch slot** per selected channel (brief may still be written/refined by orchestrator), **or** first orchestrator step is constrained to fill briefs for all materialized channels before optional multi-hop.
- LLM must **not** un-select a channel (no `skip_rag` because “history already answered”).

This is **not** “prompt says please retrieve.” It is **product caps → fixed worker presence.**

### 7.2 Completion invariant (assert)

Before `delegate_chat(mode=synthesize)` or any successful turn end with `caps ≠ ∅`:

| Materialized channel | Required record |
|----------------------|-----------------|
| rag | ≥1 finished `delegate_rag` / RagWorker run (`ok` \| `empty` \| `error` all count) |
| search | ≥1 finished SearchWorker run |

If missing:

- **Reject** early finalize; **run a default brief** for the missing channel (implementation default), or fail closed with internal error + log. Prefer **default brief + run** over silent chat-only.

**Why keep this if materialization exists?**

- Defense against implementation bugs, partial migrates, or future “orchestrator free tool” regressions.
- Cheap invariant in tests: dual + doc_scope must show rag dispatch in telemetry.

Web-only packs **never** satisfy the rag invariant (and vice versa). Per-channel ledgers replace the old `mode.id == "rag"|"search"` evidence fallthrough (`rag+search` → always true).

### 7.3 Synthesize policy (Chat exit — result handling)

Applies **after** workers have run (or returned empty/error). Orthogonal to §7.1–7.2.

| Situation | Chat behavior |
|-----------|----------------|
| All required packs `ok` with items | Grounded answer; dual merge as instructed |
| One channel `empty` / `error`, other usable | **Partial answer** from usable packs + **explicit notice** (which side failed/missed) |
| Channel ran, `empty`, doc_scope was set | Say **未命中 / 未检索到** — **not** “用户未提供正文/报告” |
| Channel never ran | Should not reach Chat synthesize if §7.2 holds; if it does, treat as engineering defect |

**Forbidden product copy class:** attributing miss to “user didn’t paste the document” when workspace retrieval was attempted or doc was in scope.

Partial notice examples (illustrative, not final i18n):

- “工作区未命中相关段落；以下主要基于网页来源。”
- “网络检索失败；以下仅基于工作区文档。”

### 7.4 What we deliberately do **not** do

- Stack three prompt paragraphs that all say “you must search.”
- Re-use union-mode `require_evidence` with a single boolean for dual.
- Let Chat open `web_search` / codegen to “fix” a skipped channel (orchestrator re-dispatch only).

---

## 8. Contracts (sketch)

### 8.1 `task_brief` (orchestrator → worker)

```json
{
  "goal": "总结立项报告中的方案结构、模块与技术选型，供与最佳实践对比",
  "constraints": ["仅工作区已选文档", "输出要点列表，不要最终长文"],
  "focus_terms": ["可选", "从上一轮缺口提取"],
  "max_items": 20
}
```

`goal` **required**. Workers treat missing goal as invalid dispatch.

### 8.2 `EvidencePack` (worker → orchestrator / chat)

```json
{
  "channel": "rag",
  "status": "ok",
  "dispatch_id": "…",
  "task_brief": { "goal": "…" },
  "items": [
    {
      "id": "chunk-uuid-or-web-index",
      "title": "optional",
      "text": "snippet",
      "score": 0.0,
      "uri": "optional for web"
    }
  ],
  "notes": "optional short bullets for orchestrator",
  "error": null
}
```

`status`: `ok` | `empty` | `error`.

### 8.3 `delegate_chat` payload

```json
{
  "mode": "direct",
  "user_query": "…",
  "instruction": "optional orchestrator guidance for synthesize",
  "packs": [],
  "partial_notices": []
}
```

```json
{
  "mode": "synthesize",
  "user_query": "…",
  "instruction": "对照文档要点与网页最佳实践做逐项差距分析",
  "packs": [ /* EvidencePack */ ],
  "partial_notices": ["search: empty"]
}
```

Chat maps packs into existing unified synthesis contract / prose path (see UNIFIED_SYNTHESIS_CONTRACT). Prefer **thin** peel of envelopes; citation hygiene remains product-visible concern of chat output, not worker essays.

---

## 9. Budgets and isolation

| Loop | Budget meaning |
|------|----------------|
| Orchestrator | Max dispatch / think rounds (e.g. 4–8); not shared with worker tool rounds |
| RagWorker | RAG mode iterations (from `rag.yaml` or successor config) |
| SearchWorker | Search mode iterations |
| Chat | Single shot (or stream) per `delegate_chat`; optional one repair for JSON contract |

Dual cost ≈ orchestrator rounds + sum(worker runs) + one chat synthesize — **clearer** than one unioned `max_iterations` spent entirely on `web_search`.

**Context isolation:** workers do not see the other channel’s full tool transcripts unless orchestrator copies notes into the next brief. Chat sees packs, not raw multi-agent tool dumps by default (size-cap packs).

---

## 10. Mapping to current codebase

| Current | Future role |
|---------|-------------|
| `ConversationApp` / pipeline execute | Unchanged entry; post-preflight runs orchestrator host |
| `CapabilitySet` / `capabilities[]` | Product gate → which workers **may/must** be used |
| `mode_assemble` tool/skill **union** | **Retire for agent execute**; replace with graph of workers + chat |
| `modes/rag.yaml` + capability-rag | RagWorker config + protocol |
| `modes/search.yaml` + capability-search | SearchWorker config + protocol |
| `modes/chat.yaml` + chat synthesis skills | Chat agent direct / synthesize |
| `agent-base.md` | Thin **orchestrator** system facts (or rename `orchestrator-base.md`) |
| `ReActLoop` | Reused **three times** with different mode configs: orchestrator, rag worker, search worker; chat may use prose/JSON complete without full tool loop |
| `auto_fallback` dense_retrieval | Inside RagWorker only |
| `user_context` skill | Catalog Use when; available on chat and/or orchestrator, not a cap tag |
| Progress SSE | New phases: `delegate_rag`, `delegate_search`, `orchestrator_plan`, `compose` (chat) |

**Iron rules (AGENTS.md):** no new AppState business methods; logic in domain (`app-chat` / `agent-loop` modules such as `orchestrator/`). Write lane untouched.

---

## 11. Prompt / disclosure principles

Aligned with prior product direction:

1. **Orchestrator prompt:** identity + paradigms + how to write `task_brief` + multi-hop; **no** answer style; **no** channel retrieval recipes; do **not** restate “you must enable RAG” (that is §7.1 code).  
2. **Worker prompts:** channel protocol only (how to retrieve / what to return as EvidencePack).  
3. **Chat prompts:** direct or grounded synthesize + §7.3 partial/未命中 policy; style/writing skills attach **here**.  
4. **Do not** re-paste synthesis contracts into orchestrator if chat/synthesis phase injects them.  
5. Tool recipes live in **skill/tool catalog** (e.g. `user_context`), not long base essays.

---

## 12. Observability and acceptance

### 12.1 Must-log / must-test scenarios

| Case | Expect |
|------|--------|
| `caps=[]` | No rag/search workers; chat direct |
| `caps=[rag]`, doc in scope | ≥1 rag dispatch; chat synthesize; no “未提供正文” if retrieval ran |
| `caps=[search]` | ≥1 search dispatch; chat synthesize |
| `caps=[rag,search]`, compare-style query | Both channels attempted; multi-hop allowed; chat final only |
| Dual search empty, rag ok | Partial answer + notice; not silent web-only success pretending full dual |
| Dual rag empty, search ok | Partial + notice |
| Budget | Orchestrator cannot exhaust budget on web tools it does not have |
| Orchestrator synthesize emits `[[cite:]]` / `[[web:n]]` | `response.citations` non-empty; doc + web kinds resolved from worker tool results |

### 12.2 Regression for the incident class

Reproduce: dual + doc_scope + “报告与最佳实践差距”  
**Fail if:** no `delegate_rag` / rag worker tools and answer claims user never provided document.  
**Pass if:** rag worker ran; answer uses doc evidence or explicit **未命中** notice plus optional web.

---

## 13. Migration strategy

### Wave O1 — skeleton + §7.1–7.2 + Chat exit (B)

- Orchestrator host: **delegate_rag / delegate_search / delegate_chat** (workers may wrap existing mode loops).  
- **§7.1** materialize from caps; **§7.2** completion invariant; **§7.3** minimal Chat policy strings.  
- Disable capability **union** path for product execute when flag on.  
- Chat always final (B).

### Wave O2 — paradigms + multi-hop quality

- Orchestrator prompt + eval for serial/parallel/gap re-search.  
- EvidencePack size limits; progress events.  
- Per-channel budgets; richer partial notices.

### Wave O3 — cleanup

- Remove dead union assemble paths; rename prompts; ADR amendment if needed (ADR-0006/0007 pointer).  
- Optional: cache identical briefs within a turn.

**Rollback:** feature flag `agent_orchestrator_v1` → previous assembled ModeConfig union (degraded dual behavior known).

---

## 14. Explicit non-goals revisited

- Frontend tag “编排 / Chat agent”.  
- Sub final answers in single-cap “for speed.”  
- Orchestrator with full tool_pool union.  
- One-shot plan as the **only** orchestrator (plan may be an internal artifact of the loop, not a substitute for multi-hop).

---

## 15. Decision log (this design)

| Topic | Decision |
|-------|----------|
| Final answers | **B:** always Chat agent |
| Orchestrator | Agent loop; **allocate only**; paradigms (serial/parallel/hierarchical…) |
| Subagents | Retrieve only → EvidencePack |
| Caps empty | Direct chat via Chat agent |
| Caps set | **§7.1 materialize** workers + **§7.2** completion invariant (not prompt-only “force”) |
| Dual partial | **§7.3** Chat synthesize: single-side + notice; 未命中 ≠ 未提供 |
| Frontend | Unchanged capability tags (RAG/Search) |

---

## 16. Next artifact

Implementation plan: [`ORCHESTRATOR_SUBAGENT_CHAT_PLAN_2026-07-16.md`](./ORCHESTRATOR_SUBAGENT_CHAT_PLAN_2026-07-16.md).

---

## 17. Summary

Replace **one unioned ReAct brain** with:

1. **Orchestrator agent loop** — topology + targeted briefs + re-dispatch; no channel execution; cannot un-select caps.  
2. **Channel workers** — RAG / Search isolation; evidence only.  
3. **Chat agent** — sole user-facing answer (direct or synthesize); not a product capability tag.  
4. **§7 three layers** — materialize / complete / synthesize policy (orthogonal, not triple “force search”).

Multi-hop flexibility stays in the orchestrator loop; dual “web-only / 未提供报告” is closed by **graph structure + invariants + Chat copy policy**, not by stacking more union prompts.
