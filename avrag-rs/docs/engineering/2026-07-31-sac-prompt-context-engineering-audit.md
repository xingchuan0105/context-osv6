# SaC prompt pack — context-engineering audit

**Date:** 2026-07-31  
**Scope:** `avrag-rs/prompts/**` as loaded on the single-agent (SaC) product path, plus retired multi-agent packs.  
**Standard:** latest public harness / context-engineering practice (primarily Anthropic 2025–2026 engineering posts), plus repo law (third-person environment language; model+skill stop; no host semantic completeness gates).

---

## 1. Best-practice baseline (web, 2025–2026)

Sources used for this audit (not exhaustive):

| Source | Core claim used here |
|--------|----------------------|
| [Anthropic — Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) (2025-09) | Context = finite attention budget; engineer **what enters the window**, not only wording of a static prompt. System prompts at the **right altitude** (not brittle if-else scripts, not vague slogans). Section structure; **minimal high-signal** tokens; diverse few-shots not laundry lists. Just-in-time retrieval over stuffing. Write / Select / Compress / Isolate. |
| [Anthropic — Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) (2025-11) | Harness = environment + artifacts + completion signals. Failure modes: one-shot everything; early “done”; dirty state between sessions. Prefer **observable state** (progress files, feature lists) over re-prompting obedience. |
| Industry synthesis (2026 harness engineering writeups) | “Agents aren’t hard; the harness is.” Separate generation vs evaluation when self-grading fails; done-conditions as contracts. |
| Repo law (`AGENTS.md` / `prompts/README`) | Environment facts not imperatives; prompts only under `prompts/`; stop ownership = model + skill. |

### Audit rubric (per asset)

| Dimension | Pass means |
|-----------|------------|
| **R1 World model** | Tools, sandbox, returns, limits, failures are facts the model can use |
| **R2 Evidence semantics** | What counts as support; unknown vs absent; cite protocol |
| **R3 Task / coverage state** | Multi-entity, half-coverage, stop conditions as environment |
| **R4 Altitude** | Heuristics + ontology, not step scripts or empty “be careful” |
| **R5 Voice** | Third-person / observational; few 必须/不要 as pure bans |
| **R6 Density** | High signal per token; no golden-set leakage |
| **R7 Layer fit** | Right layer (system vs skill vs loop obs vs retired) |

---

## 2. What the model actually sees (SaC)

```
assemble_mode(caps)
  pure chat  → chat-base
  rag        → capability-rag  + skill codegen (+ how-to-read-tables)
  search     → capability-search + skill search
  dual       → both capability manuals + both retrieve skills
+ progressive skill_request (memory / metadata / writing / format)
+ loop/* observations on sandbox / budget events
+ tool results as <code_execution_result> …
```

Product entry: `dispatch_agent_mode` → **no** orchestrator-base, **no** Answer-phase pack, **no** worker handoff JSON requirement.

---

## 3. Inventory & scores

### 3.1 Live system voices (`prompts/orchestrators/`)

| File | Role | CE score | Notes |
|------|------|----------|-------|
| `capability-rag.md` | Workspace task contract | **7.5/10** | Strong evidence closed-loop + multi-entity coverage + few-shots. Still light on truncation / method risk / claim ledger. Some residual “常见做法是再写代码” (mild procedure). |
| `capability-search.md` | Web task contract | **6.5/10** | Environment thin: bilingual/default rules lean imperative-ish (“默认…至少”). Missing empty-result semantics, source-conflict model as environment, cost of fetch. |
| `chat-base.md` | Pure chat | **8/10** | Clear capability boundary; memory protocol as emit contract. Short and right altitude. |
| `write-refine-system.md` | Write-refine mode | **5.5/10** | Explicit step list + “只调用下列三者之一” — **workflow prompt**, not pure environment. Acceptable for tool-gated refine loop, but fails CE voice standard if applied to SaC. Out of SaC chat scope. |

**Folder name `orchestrators/` is misleading** post-SaC; treat as system voices. Multi-agent files moved out 2026-07-31.

### 3.2 Live retrieve / support skills (`clusters/`)

| Asset | CE score | Strengths | Gaps |
|-------|----------|-----------|------|
| `codegen/SKILL.md` | **6.5–7/10** | Sandbox world model, API table, dense/lexical/grep routing, SELECTED protocol | Truncation (`truncated`), empty hits, tool credibility/risk, stop-on-coverage; still slight “不要改用点选式” ban voice |
| `codegen/reference/how-to-read-tables.md` | **9/10** | Ontology + few-shot; record model; total_hits/表序 — **reference standard for the pack** | Keep as core; one-line hard link from codegen top |
| `search/SKILL.md` | **5.5/10** | API + bilingual example | Numbered “策略 1–6” is pipeline tone; credibility as facts OK but framed as steps |
| `memory/SKILL.md` | **6.5/10** | Default history window as environment | Numbered procedure + “不要假设…” bans |
| `memory/reference/anaphora.md` | **7/10** | Pattern ontology | Mild step list |
| `metadata/SKILL.md` | **7.5/10** | Injection shape, null semantics | Protocol emit is necessary (not pure CE prose) |
| `writing/*` / `format/*` | **7–8/10** | Progressive disclosure; style ≠ evidence | Some “不要去掉引用” ban lines — could be “引用标记是材料的一部分” |
| `heavytail-*` | **n/a SaC** | Tool-gated refine | Workflow-style by design |
| `index` / `workspace-create` | **n/a SaC chat** | MCP helpers | Out of scope |

### 3.3 Loop observations (`loop/`)

| Class | Files | Verdict |
|-------|-------|---------|
| Good observations | `blocks-skipped`, `codegen-no-output`, `codegen-sandbox-error`, `codegen-untrusted-prefix`, `format-hint-*`, `retrieval-summary`, budget carryover | Align with CE: tagged facts about runtime |
| Mixed | `budget-exhausted-final` | Still mentions worker “summary/coverage/gaps” structure — **orchestrator leftover** on SaC prose path |
| User-facing fallbacks | `contract-violation-*`, `degraded-no-evidence-*` | OK as final user strings; “请重试” is user voice not model CE |
| Legacy / skill-only era | `no-chunk-*`, `retrieval-failed-final`, `partial-evidence-insufficient`, `synthesis-repair` | Still in `prompt_assets` / include paths; **policy may not inject** after require_evidence retirement — deadweight risk and wrong mental model if re-enabled |

### 3.4 Retired this audit (archived)

Moved to `prompts/deprecated/orchestrator-multiagent/`:

- `orchestrator-base.md`
- `capability-rag.dispatch.md` / `capability-search.dispatch.md`
- `product-answer-base.md`
- `answer-from-workspace.md` / `answer-from-web.md` / `answer-dual-source.md`

Already deprecated earlier: `deprecated/monomode-system/`, `deprecated/synthesis/`.

**Note:** dual-source conflict rules in `answer-dual-source` are **high-value CE content** not yet fully lifted into live `capability-rag` + `capability-search` dual path. **Do not delete knowledge — port environment facts into SaC manuals before any hard delete.**

### 3.5 Non-SaC / human only

- `pipeline/*` — ingestion workers; separate eval standard  
- `_backups/*` — historical; consider eventual merge under `deprecated/`  
- empty `synthesis/` dir — can remove when convenient  

---

## 4. Cross-cutting findings

### 4.1 Direction is correct

Pack already moved from “must retrieve / must not invent” toward:

- sandbox as world model  
- evidence only from code returns  
- SELECTED / web cite as protocols  
- table ontology  

This matches Anthropic’s “right altitude” and “tools as environment contract.”

### 4.2 Still tool-manual-heavy, not full harness

Missing (highest impact for reliability):

1. **Truncation / empty / zero-hit semantics** as first-class environment (`truncated`, empty list vs uncalled).  
2. **Stop / completion conditions** owned by skill (coverage of claims; unknown ≠ corpus absence).  
3. **Method risk model** (dense ≈ concept bias; lexical ≈ literal; grep ≈ pattern brittleness).  
4. **Dual-source conflict environment** (retired answer-dual-source content).  
5. **Runtime state injection** (budget remaining, aliases seen, last truncated) — skill can only explain what host surfaces.  
6. **Claim–evidence mapping** as light semantics (not forced JSON ledger every turn).

### 4.3 Voice debt (imperative / pipeline)

| Location | Issue | Prefer |
|----------|-------|--------|
| `search` strategy 1–6 | Numbered pipeline | “Default environment: bilingual queries are available; empty twice → …” |
| `memory` 1–4 + 不要 | Steps + bans | Window size + ambiguity → clarify as state |
| `capability-search` 默认至少 | Soft order | Bilingual as recommended coverage fact |
| `codegen` 不要改用点选 | Ban | “Retrieval entry is client.*; same-named native tools are not this sandbox” |
| `budget-exhausted-final` | Worker handoff fields | SaC: prose + SELECTED / [[web:n]] only |

### 4.4 Layer confusion

| Symptom | Fix |
|---------|-----|
| Folder `orchestrators/` holds SaC system voices | Rename later to `system/` or `voices/` (code touch) |
| Dual-source rules only on retired Answer pack | Port to capability manuals |
| Loop still teaches `summary/coverage/gaps` | Align with SaC DirectAnswer |
| Host multi-agent code still loads deprecated paths | OK for tests; document RETIRED; do not re-enable product entry without ADR |

### 4.5 Orchestrator formal retirement (this change)

| Done | Deferred |
|------|----------|
| Multi-agent prompt pack → `deprecated/orchestrator-multiagent/` | Delete `app-chat/orchestrator/**` crate surface |
| `modes/orchestrator.yaml` marked RETIRED | Full dead-code purge of brain/host |
| Paths in host/brain/prompt_leak retargeted | `prompt_leak` may drop retired bodies later |
| `prompts/README` + deprecated READMEs | Rename `orchestrators/` directory |

Product entry was already SaC-only; this makes **prompt ownership** match that fact.

---

## 5. Priority roadmap (CE upgrades, no host semantic gates)

### P0 — environment facts into live SaC pack

1. **codegen:** open with evidence authority + unknown state (strong rewrite already drafted in prior review).  
2. **codegen:** document `truncated`, empty results, stderr, 0 `total_hits`.  
3. **capability-rag:** stop condition = claims covered **or** explicit 未覆盖; half-coverage already partially present.  
4. **Port dual-source conflict** from retired `answer-dual-source` into dual-capable manuals (short section).  
5. **loop `budget-exhausted-final`:** drop worker handoff field language.

### P1 — risk models & table promotion

6. Method risk / applicability (not “you must use X”).  
7. One hard structural summary of tables in codegen + keep reference.  
8. Rewrite `search` strategy list into environment table.  

### P2 — state & coverage

9. Optional light claim-coverage semantics (skill text only).  
10. Host: surface structured observations (truncation flags, alias set) if not already in tool JSON — skill documents meaning only.  
11. Soft-archive unused loop files once call sites confirmed dead (`no-chunk-*` if never injected).

### P3 — hygiene

12. Rename `orchestrators/` → `system/` when willing to touch mode_assemble + yaml + tests.  
13. Prune `_backups/` into `deprecated/backups/`.  
14. Remove empty `synthesis/`.  
15. Later: delete or fence `orchestrator` Rust module behind `cfg(test)` / feature flag.

---

## 6. Scorecard (pack-level)

| View | Score |
|------|-------|
| SaC live pack as **tool world model** | **8/10** |
| SaC live pack as **context-engineering harness** | **6.5–7/10** |
| Table ontology alone | **9/10** |
| Multi-agent orchestrator prompts | **Retired** (historical, not scored for product) |
| Path to ~9/10 CE | P0–P1 environment facts + dual-source port + loop alignment + selective runtime observations — **not** more imperative checklists |

---

## 7. Explicit non-goals (align with repo)

- Reintroduce host “answer completeness” or `require_evidence` hard gates.  
- Hardcode golden-set entities into skills.  
- Force per-turn claim-ledger JSON as host-validated protocol without a new ADR.  
- Expand codegen into a full DAG state machine document.

---

## 8. Changes landed with this audit

1. Archived multi-agent prompts → `prompts/deprecated/orchestrator-multiagent/` (+ `RETIRED.md`).  
2. Updated code load paths (host, brain, prompt_leak, `modes/orchestrator.yaml`).  
3. Refreshed `prompts/README.md`, `deprecated/README.md`, chat-base description.  
4. This document.

## 9. P0 implementation (same day follow-up)

| Asset | Version | What landed |
|-------|---------|-------------|
| `clusters/codegen/SKILL.md` | 3.0 | Evidence authority; empty/truncation/failure table; method risk columns; table ontology summary + reference; client.* as sandbox contract (no ban tone) |
| `orchestrators/capability-rag.md` | 3.0 | Claim coverage states; stop when closed; dual-source port from retired answer-dual-source; dual cite example |
| `orchestrators/capability-search.md` | 3.0 | Empty/credibility/conflict environment; dual-source when workspace on |
| `clusters/search/SKILL.md` | 3.0 | Strategy 1–6 → environment factors table |
| `loop/budget-exhausted-final.nudge.md` | — | SaC prose + SELECTED / `[[web:n]]` (no worker handoff fields) |

**Estimated pack CE after P0:** ~7.5–8.0/10.

## 10. P2 implementation (pre–golden-14)

| Item | Status |
|------|--------|
| `memory` + `anaphora` environment voice | Done (v3.0) |
| `writing` / `format` ban-tone → scope facts | Done (v3.0) |
| Dead `no-chunk-*` → `deprecated/loop-legacy/` | Done; `prompt_assets` loads legacy only for tests |
| Runtime observation: `retrieval_summary` + alias / truncated / grep0 | Done in `iteration_codegen::retrieval_callouts` |
| capability-rag documents observation tags | Done (v3.1) |
| Light claim-coverage (states table) | Already in capability-rag v3.0 |
| Dual-source environment | Already in capability-rag/search v3.0 |
| `orchestrators/` rename | Deferred (code churn) |
| golden-14 E2E | **Not run in this wave** (explicit stop) |

**Estimated pack CE after P2:** ~8.0–8.5/10.

## 11. P0–P3 drift fix (2026-07-31 later)

| Priority | Delivered |
|----------|-----------|
| P0 | `prompts/README` three-layer synonym map; rag/search yaml comments fixed; assemble comment; auto_fallback documented as host-only |
| P1 | agent-base + knowledge-base + web execution-surface env (fake tool / fake result not evidence); dual few-shot in capabilities |
| P2 | `loop/README` documents `codegen-*` as sandbox implementation codename |
| P3 | writing/format disclosed on retrieve when request has hints (SaC ProseOnly); orchestrator isolation doc + pipeline_steps pointer |

Isolation doc: `docs/engineering/2026-07-31-sac-orchestrator-isolation.md`.
