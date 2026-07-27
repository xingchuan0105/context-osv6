# Prompt CDS v1.1

Context Disclosure System (CDS) prompt layout for the unified ReAct agent, ingestion worker, and chat postprocess.

See [docs/agents/cds-v1.1.md](../docs/agents/cds-v1.1.md) for the full spec.

## Layout

| Family | Path | Registry | Typical callers |
|--------|------|----------|-----------------|
| **A — Clusters** | `clusters/<id>/` | `PromptRegistry` (cluster id) | ReAct Index → Load → Reference |
| **B — Loop assets** | `orchestrators/`, `synthesis/` | `PromptRegistry` | Orchestrator system prompt, mandatory synthesis prompts |
| **C — Pipeline** | `pipeline/*.system*.md` | **Not registered**; `include_str!` / hot reload | Worker ingestion, chat postprocess |
| **Templates** | `templates/*.tmpl` | Not registered | User-side templates paired with pipeline system prompts |

### Capability multiselect + `user_context` (2026-07-15, updated 2026-07-20)

- Product assemble (P1 — no shared base): pure chat = `chat-base.md` (self-contained); with capabilities = `capability-rag.md` and/or `capability-search.md` only (each self-contained; `agent-base.md` retired).
- Product dispatch: pure chat never enters the orchestrator; **any** rag/search selection always uses the orchestrator host (workers + chat exit). There is no flat single-agent fallback (`AGENT_ORCHESTRATOR_V1` retired 2026-07-20).
- Worker final message = the brief's `internal_worker_handoff_v1` JSON (PR-A 2026-07-20): capability paths run `ProseOnly` + early-stop; `rag-answer` / `search-answer` / `grounded-answer` / `synthesis/chat.md` are **no longer mandatory** on any main path — they stay on disk for `token_budget` `include_str!` references only.
- Answer phase (chat exit) pack: `product-answer-base.md` (voice + memory protocol + grounding) + `answer-from-workspace` / `answer-from-web` (+ `answer-dual-source`) chosen by the turn's actual materials (PR-B 2026-07-20; `answer-follow-brief` merged into `product-answer-base`, full `chat-base` no longer stacked).
- AnswerOnly / Answer phases share the utility tool whitelist (`user_context` + `calculator` + `weather_query`; OQ-Tools 2026-07-20); retrieval/delegate tools are never in it.
- Orchestrator (V2 brain) system: `orchestrator-base.md` + the `## 给任务分配者` section of each opened channel's capability manual + per-round runtime state.
- Base tool `user_context` (clock + MaxMind city) is always in the agent tool pool; optional geo via env `GEOIP_CITY_DB_PATH` (see `avrag-rs/.env.example`). Without the DB, geo degrades to `confidence: none`.
- Product write (`agent_type=write`) is offline (`write_mode_disabled`).

## Clusters (A)

| Id | Mode | Notes |
|----|------|-------|
| `codegen` | RAG retrieve | Atomic bundle: SKILL + all `reference/` at Round 0 |
| `writing` | Synthesis | Default neutral prose; load ≤1 reference via `writing_ref` |
| `format` | Synthesis | Output shape; load ≤1 reference via `format_ref` |
| `memory` | Retrieve | Conversation memory helpers |
| `search` | Search retrieve | Search-only cluster |

## Pipeline (C)

| File | Used by |
|------|---------|
| `pipeline/summary-generation.system.v1.md` | `llm/summary.rs`, worker |
| `pipeline/summary-generation-finalize.system.v1.md` | `llm/summary.rs` |
| `pipeline/triplet-extraction.system.md` | worker triplet batch |
| `pipeline/section-index.system.v1.md` | worker TOC LLM fallback |
| `pipeline/user-profile-extraction.system.md` | chat postprocess |

Templates: `templates/summary-user.tmpl`, `summary-finalize-user.tmpl`, `section-index-user.tmpl`.

## Deprecated

`deprecated/atomic-tools/` holds retired tool prompt docs from before ADR-0007. They are **not** scanned by `build.rs`, **not** in `PromptRegistry`, and **not** used at runtime. Native tool schemas live in Rust `SkillComponent` + mode `tool_pool` → `CapabilityRegistry`; RAG retrieval goes through `codegen` SDK calls and server-side fallback.

`deprecated/monomode-system/` holds the retired per-mode monomode system prompts (`rag-system.md` / `search-system.md` / `chat-system.md`, retired 2026-07-20 P2). The main path no longer uses them: pure chat = `chat-base.md`; capability workers = `capability-rag.md` / `capability-search.md`; orchestrator = `orchestrator-base.md` + each channel's `## 给任务分配者` section; the chat exit appends `answer-from-workspace` / `answer-from-web` (+ `answer-dual-source`) chosen by the turn's actual materials on top of `product-answer-base.md`. Same not-scanned / not-registered / not-used-at-runtime status as `atomic-tools/`.
