# Design: ReAct Orchestrator + Shared Evidence Store (V2)

> **SUPERSEDED** — 本文描述的 orchestrator / worker 多 agent 架构已被取代：2026-07-30 起产品路径改为单 agent（SaC 设计，见 `docs/plans/2026-07-30-sac-sdk-single-agent-design.md`），orchestrator 代码已物理删除（commit `7f2d182d`）。本文仅作历史记录。（横幅添加于 2026-08-02 文档体系梳理）

**Date:** 2026-07-18
**Status:** Proposed (product owner direction + best-practice review)
**Supersedes:** O1 structural host (`run_orchestrated_turn` first-wave fan-out). Keeps §7 channel-integrity invariants and Option B (chat sole user-facing exit) from [ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md](./ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md).
**Motivating incident:** 2026-07-17 manual run (` caps=[rag,search] `, doc = 《数字化转型IT立项报告》): search pack empty (raw user query is an invalid web query), answer misread the doc genre (chunks without identity), fabricated `[[web:n]]` markers for an empty channel. Root cause: no query decomposition, lossy evidence interfaces, no shared evidence identity.

---

## 1. Product-owner direction (locked)

1. **Orchestrator is a ReAct loop**, not a plan-then-execute planner: it takes **one action at a time** (dispatch / decide), observes the result, and chooses the next action. The path is **adjustable**; no full task graph is committed up front.
2. **Workers and the chat exit are also ReAct loops** (4 agent loops total), each deciding its internal path and when to exit within its own budget.
3. **All agents share one evidence store.** Every retrieved unit (doc chunk, web page) gets a **stable reference id**; agents pass **references, not raw text** between loops.
4. **Typical task flow** (compare-doc-vs-best-practice class): orchestrator resolves the referent ("这篇文档" → doc identity) → `delegate_rag` (what the doc *is*, its structure, its key content) → `delegate_search` (best practices **for the parts RAG found**) → `delegate_chat` with a **brief**: original question + how it was decomposed + what evidence exists (by reference).

## 2. Best-practice comparison

| User decision | Industry anchor | Verdict |
|---|---|---|
| Step-wise ReAct orchestrator, adjustable path | Anthropic multi-agent research system: Lead Researcher analyzes query, records strategy **in memory**, gathers subagent results, then *re-plans*: "decides if further work is required… strategy can be refined" ([ByteByteGo summary](https://blog.bytebytego.com/p/how-anthropic-built-a-multi-agent)) | Aligned. Keep Anthropic's "strategy note in working memory" — a few lines the orchestrator re-reads each round, *not* a committed task graph. |
| Subagents = ReAct loops with own exit | Same: subagents "search, evaluate results, and refine queries independently" in isolated contexts; Claude Code subagents formalize isolated context + tool whitelist | Aligned. Workers keep ReAct; their **exit contract changes** (§3.4). |
| Shared evidence store, pass-by-reference | Blackboard architecture (Hayes-Roth 1985; [Agentic AI Wiki deep dive](https://menuagentic.com/deep-dives/multi-agent-systems/shared-memory-and-blackboard/)): one shared workspace collapses O(n²) messages to O(n) reads/writes and gives a single observation point | Aligned, with discipline: **append-only, monotonic ids, turn-scoped** — avoids the classic blackboard hazards (write contention, stale reads) by construction (§3.3). |
| Chat sole exit with a brief | Anthropic separates synthesis from retrieval; additionally has a **Citation Agent** re-checking every claim against sources | Aligned; adopt citation verification as a **post-check first**, agent later (§3.6). |
| ReAct over plan-first | [ReAct vs plan-first comparisons](https://dev.to/jamesli/react-vs-plan-and-execute-a-practical-comparison-of-llm-agent-patterns-4gh9): ReAct = adaptive recovery, more calls; plan-first = fewer calls, risks stale plan on discovery-heavy tasks. [Anthropic, *Building Effective Agents*](https://www.anthropic.com/research/building-effective-agents): orchestrator-workers fits tasks where "subtasks aren't pre-defined, but determined by the orchestrator based on the specific input" | Research/compare queries are discovery-heavy → ReAct justified. Mitigate cost with budgets (§3.7); Anthropic reports ~**15× chat token usage** for full multi-agent — accept for this query class, keep pure-chat path cheap. |
| Pass-by-reference between agents | Anthropic (via [multi-agent context-loss analysis](https://github.com/mareurs/codescout/blob/master/docs/research/multi-agent-context-loss.md)): subagents "**store work in external systems, then pass lightweight references back to the coordinator**" — avoids the telephone game; coordinator reads output artifacts directly | Aligned — this is exactly the store + eid design; no change needed. |
| Reference stub + on-demand read | [shisad SECURITY.md](https://github.com/shisa-ai/shisad/blob/main/docs/SECURITY.md): content-addressed evidence store; context carries only a stub `[EVIDENCE ref=… source=… summary=…]`; model calls `evidence.read(ref_id)` to re-examine | Independent convergence on `evidence_fetch`. Bonus: keeps **untrusted web content out-of-band** — matches this repo's existing `untrusted_input.rs` / content-guard stance; adopt the stub format (§3.3). |
| Workers as leaf agents | [Hermes delegation docs](https://hermes-agent.nousresearch.com/docs/user-guide/features/delegation): subagents are leaves by default and **cannot delegate further**; depth caps prevent runaway recursion | Adopt as a hard guard (§3.2): workers/chat have no dispatch tools. |

## 3. Optimized design

### 3.1 Roles

| Agent | Loop | Owns | Must not |
|---|---|---|---|
| Orchestrator | ReAct (max dispatch rounds) | Query understanding (coreference, doc orientation intent), next-action choice, brief writing, re-dispatch on gaps | Call retrieval tools; write user prose; un-select a materialized channel |
| RagWorker | ReAct (rag budget) | Doc identity/structure orientation **first**, then targeted extraction; returns evaluated evidence (§3.4) | Touch web tools; write the final answer |
| SearchWorker | ReAct (search budget) | Web retrieval for the **orchestrator-written query** (never the raw user utterance), **bilingual zh+en sub-queries by default** (hard rule, §3.4); returns evaluated evidence | Touch rag tools; write the final answer |
| Chat exit | 1-shot + 1 repair (not full ReAct in V2) | Sole user answer: interpretation statement, synthesis, citation markers **as store references** | Call retrieval tools (needs more → orchestrator re-dispatch) |

Deviation from "all four are ReAct": chat stays 1-shot+repair in V2 because its inputs are already digested; promoting it to a loop is a later, cheap upgrade (the host interface is loop-agnostic). Flag if product feels strongly.

### 3.2 Orchestrator loop mechanics

- Tools (host-intercepted, **not** registered on the global catalog): `delegate_rag`, `delegate_search`, `delegate_chat`, `evidence_fetch`, `finish_with_chat`.
- **Briefs are LLM-written, prompt-guided.** De-referencing ("这篇报告" → concrete doc identity/topic), channel-appropriate sub-questions, and bilingual query generation are the orchestrator's **reasoning**, instructed by `orchestrator-base.md` and the capability manuals — **no rule-based de-contextualization / query-rewriting code exists or may be added**. Code owns only: materialization, finish-gates, the store, and marker finalization. The sole code-generated brief is the finish-gate fallback (`default_brief` = the raw user query, policy-free).
- Each round the model sees: strategy note (its own prior notes), dispatch ledger (channel, brief, status, evidence ids produced), and remaining budget. It outputs **one** tool call.
- **Hard guards (code, not prompt):**
  - §7.1 materialization: selected channels must be dispatched ≥1 time before `delegate_chat` (§7.2 invariant becomes the loop's finish-gate; missing → host injects a default brief and continues, as today).
  - Loop-shape guards: dedupe identical (channel, brief-hash) dispatches; cap re-dispatches per channel (default 2); budget exhaustion → forced `delegate_chat` with what exists + partial notices.
  - **Leaf-agent rule:** workers and chat have **no dispatch tools** — delegation depth is 1 (Hermes/Claude Code convention); recursive fan-out is not representable.
  - Chat cannot be dispatched with zero evidence when caps ≠ ∅ (finish-gate again).

### 3.3 Shared Evidence Store (the blackboard)

Per-turn, in-memory, owned by the host. **Append-only; ids monotonic; read-by-id for every agent.**

```rust
struct EvidenceStore { entries: Vec<EvidenceEntry> }   // eid = "E{index+1}"

struct EvidenceEntry {
    eid: String,                 // "E7" — stable for the whole turn
    channel: Channel,            // rag | search
    kind: EvidenceKind,          // doc_chunk | web_page
    // Identity (what O1 dropped):
    doc_id: Option<String>,
    doc_name: Option<String>,    // file_name resolved once at insert
    page: Option<usize>,
    url: Option<String>,
    title: Option<String>,
    // Payload:
    preview: String,             // ≤300 chars — shown in listings
    full_text: String,           // capped (e.g. 4k chars) — only via evidence_fetch
    score: Option<f64>,
}
```

- **Insert path:** workers' raw tool results are normalized into entries **by the host** (not by the LLM), so numbering is deterministic and citation ids are real by construction.
- **Read path:** agents receive *listings* (eid, title/doc_name, preview) in their context; full text only through `evidence_fetch(eids[])` with a per-call cap.
- **Why this fixes the citation break:** today the chat sees one representation (pack JSON) while citation rebuild validates against another (raw tool_results). With the store there is a single source of truth: chat cites store ids, the host validates them against the store itself.
- **Marker scheme (V1 implementation amendment):** agents emit `[[E:id]]` **internally**; after the chat run the host rewrites valid E-ids to existing product markers — `[[cite:chunk_id]]` for doc chunks, `[[web:n]]` (renumbered in order of appearance) for web — and builds `contracts::Citation` from the same store entries. Frontend marker parsing stays unchanged; dangling E-ids and off-protocol raw markers (`[[web:1]]`, `[[cite:fake]]`) are stripped with a warning. This preserves single-source grounding without a frontend contract change.
- **Doc-name resolution:** rag inserts join `doc_scope` → `documents.file_name` once (via `DocScopeMetadata`, which also carries `genre`). This restores the missing 《数字化转型IT立项报告》 identity for both genre judgment and chip display.

### 3.4 Worker exit contract (evaluated evidence, not chunks, not prose)

Worker synthesis is replaced by an **internal handoff** (structured, short):

```json
{
  "summary": "这份文档是IT立项报告（V1.0.3），结构：现状诊断→目标架构→实施路径→投资估算→保障措施…",
  "orientation": { "doc_kind": "立项报告/规划方案", "confidence": "high" },
  "key_facts": [{ "claim": "…", "evidence": ["E3", "E5"] }],
  "coverage": "full | partial",
  "gaps": ["未找到投资估算章节"]
}
```

- RAG worker **must orient first** (doc_profile/doc_summary/dense probes) before gap-targeted retrieval — this is what its ReAct loop decides; the brief states the requirement, the loop chooses the tools.
- Search worker receives an **orchestrator-written query** (de-referenced: "数字化转型 立项报告 最佳实践 框架 评价标准"), never the raw "这篇报告…" utterance — **and bilingual by hard rule**: every search task produces ≥1 Chinese and ≥1 English sub-query (English sources are richer for frameworks/technology/methodology; translate to industry terms first, e.g. 立项报告 → "project initiation report / IT transformation plan"). Rule pinned in `prompts/orchestrators/capability-search.md` and `prompts/clusters/search/SKILL.md`.
- Worker mode configs need an internal-handoff synthesis contract (new `internal_worker_handoff_v1` or guided prose). The current user-facing `InternalAnswerUnifiedV1` envelope is **not** reused inside workers — that was the O1 semantic leak.

### 3.5 Chat brief (orchestrator → chat)

```json
{
  "user_query": "这篇转型报告和最佳实践的差距在哪里？",
  "interpretation": "按『报告方案本身 vs 同类最佳实践』理解",   // ambiguity stated up front
  "decomposition": "RAG 定向文档为立项报告并抽取其结构要素；Search 按结构检索对应最佳实践框架",
  "evidence": {
    "doc": [{ "eid": "E3", "title": "立项报告-现状诊断", "preview": "…" }],
    "web": [{ "eid": "E9", "title": "Best practices for IT transformation plans", "preview": "…" }]
  },
  "partial_notices": ["search: empty …"],
  "policy": "引用用 [[E:id]]；空通道只能表述为未检索到；先一句话说明理解口径"
}
```

Chat may `evidence_fetch` top items (bounded) before writing. Answer markers `[[E:n]]` → host maps to citations.

### 3.6 Citation integrity

- **Post-check (V2 default):** strip `[[E:id]]` markers with no matching store entry + `tracing::warn` + metric; strip whole web-citation section when the search channel is empty (the fabricated-marker case).
- **Later (V3):** a cheap citation-verifier pass — Anthropic's Citation Agent pattern (claim↔source spot check on top N claims) — only if post-check metrics show marker drift.

### 3.7 Budgets and cost honesty

| Loop | Budget | Note |
|---|---|---|
| Orchestrator | 4–8 dispatch rounds | Strategy note ≤ 200 tokens, re-injected per round |
| RagWorker | rag.yaml iterations | Orientation + targeted retrieval |
| SearchWorker | search.yaml iterations | Per orchestrator query |
| Chat | 1 + 1 repair | Synthesize only |

Dual compare turn ≈ orchestrator rounds + worker runs + chat ≈ well above legacy union cost; Anthropic's 15× figure is the honest ceiling reference. **Pure chat (`caps=[]`) keeps today's cheap single-loop path** — the store and orchestrator only activate for `caps ≠ ∅`.

### 3.8 Observability

Dispatch ledger entries (channel, brief, status, eids out, elapsed) logged at INFO per dispatch — the 2026-07-17 "search empty, reason invisible" gap closes. Store stats (entries per channel, fetch calls) into turn metadata.

## 4. Mapping to current code

| Current (O1) | V2 |
|---|---|
| `orchestrator/host.rs` structural first wave | ReAct orchestrator loop: reuse `ReActLoop` with `modes/orchestrator.yaml` (activate the O2 artifact); host intercepts `delegate_*` / `evidence_fetch` tool calls |
| `types.rs::EvidencePack` (lossy items + 800-char notes) | `EvidenceStore` + `EvidenceEntry` (identity-complete); worker handoff `{summary, key_facts, coverage, gaps}` replaces notes |
| `workers.rs::pack_from_run` | `store.insert_from_tool_results(channel, tool_results)` (host-side, deterministic) |
| `chat_exit.rs::render_synthesize_context` (packs JSON) | chat brief §3.5 (listings + refs; full text via fetch) |
| `attach_worker_evidence` (filter by markers vs raw results) | `workers::finalize_answer_evidence` — rewrite `[[E:id]]` → product markers + citations/sources from store; dangling-marker strip |
| Worker mode configs = user-facing unified envelope | Worker internal-handoff contract (new synthesis contract kind or guided prose) |
| `modes/orchestrator.yaml` (unused O2 artifact) | Activated as the orchestrator loop config |
| §7.1/§7.2 materialize + invariant | **Kept** — as finish-gates around the LLM loop, not replaced by it |

## 5. Waves

| Wave | Outcome |
|---|---|
| **V1 — Evidence Store end-to-end** (keep structural host) | Store + eids + doc-name/genre join + host-side marker rewrite (`[[E:id]]` → product markers) + chat brief with listings + per-channel worker digests (≤2000 chars) + dangling-marker strip + dispatch INFO logs. Kills citation break + doc-identity loss without betting on LLM orchestration. **Status: implemented 2026-07-18** (`orchestrator/store.rs`, reworked `chat_exit.rs`/`workers.rs`/`host.rs`; `cargo test -p app-chat --lib` 118 green). |
| **V2 — ReAct orchestrator** | orchestrator.yaml loop with delegate/evidence_fetch interception, strategy note, finish-gates, loop guards; worker handoff contract; search-brief de-referencing emerges from the loop. **Status: implemented 2026-07-18** (`orchestrator/brain.rs`, flag `AGENT_ORCHESTRATOR_V2`, default off; batched delegates run as a concurrent wave; budget-exhaustion falls back to deterministic finish). Strategy note = refreshed per-round system state block (ledger + store stats + budget), not a separate artifact. Structured worker handoff (`internal_worker_handoff_v1` / `{summary, key_facts, coverage, gaps}`) **landed 2026-07-19** — parse in `workers::parse_worker_handoff`, surface in chat brief + delegate tool results; free-form falls back to partial. **Hardening after first live run:** store TOPK gate at ingest (per-channel caps 24/12, dedupe by locator, score-ranked retention — full-doc scans can no longer flood the shared store); dispatch distinguishes tool **Error** (检索失败, surfaces to model + log) from **Empty** (未命中); worker `req.query` = self-contained brief goal; `delegate_chat` instruction must state the 理解口径 explicitly. |
| **V3 — Quality + verification** | Citation verifier (if metrics warrant), multi-hop eval set (compare-doc class), optional chat-as-loop. |

Rationale for V1-first: the two confirmed production defects (genre misread, citation break) live in the **interfaces**, not in who writes the briefs; V1 fixes them with deterministic code and buys the telemetry to tune V2.

## 6. Non-goals

- Full blackboard DB persistence / cross-turn shared memory (turn-scoped only).
- Plan-then-execute orchestrator variant (rejected by product owner §1.1).
- Frontend capability-tag changes; Write lane; multi-agent UX beyond progress stages.

## 7. Sources

- [How Anthropic Built a Multi-Agent Research System (ByteByteGo)](https://blog.bytebytego.com/p/how-anthropic-built-a-multi-agent) — lead-agent strategy-in-memory + re-plan loop, subagent isolation, Citation Agent, ~15× tokens, 90% quality gain.
- [Anthropic multi-agent context-loss analysis (mareurs/codescout)](https://github.com/mareurs/codescout/blob/master/docs/research/multi-agent-context-loss.md) — "store work in external systems, pass lightweight references back to the coordinator"; telephone-game avoidance.
- [shisad SECURITY.md — Evidence References](https://github.com/shisa-ai/shisad/blob/main/docs/SECURITY.md) — content-addressed store, ref stub + `evidence.read(ref_id)`, untrusted content out-of-band.
- [Hermes Agent — Subagent Delegation](https://hermes-agent.nousresearch.com/docs/user-guide/features/delegation) — leaf-by-default subagents, depth caps, audit surface.
- [Anthropic — Building Effective Agents](https://www.anthropic.com/research/building-effective-agents) — orchestrator-workers: subtasks determined dynamically, not pre-defined.
- [Shared Memory & the Blackboard (Agentic AI Wiki)](https://menuagentic.com/deep-dives/multi-agent-systems/shared-memory-and-blackboard/) — O(n) shared workspace, consistency hazards (mitigated here by append-only + monotonic ids).
- [ReAct vs Plan-and-Execute comparison](https://dev.to/jamesli/react-vs-plan-and-execute-a-practical-comparison-of-llm-agent-patterns-4gh9) and [ReAct pattern analysis](https://mbrenndoerfer.com/writing/react-pattern-llm-reasoning-action-agents) — adaptive recovery vs call count.
- [LangChain — Context Management for Deep Agents](https://www.langchain.com/blog/context-management-for-deepagents) — offloading/summarization/filesystem abstraction for long-running agents (later-wave option for the store).
- [LLM-based Multi-Agent Blackboard System (arXiv 2510.01285)](https://arxiv.org/html/2510.01285v1) — central agent posts requests; subordinates self-select (rejected here: our dispatch stays explicit).
