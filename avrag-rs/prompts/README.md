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
| `pipeline/session-summary.system.md` | chat postprocess |
| `pipeline/user-profile-extraction.system.md` | chat postprocess |

Templates: `templates/summary-user.tmpl`, `summary-finalize-user.tmpl`, `section-index-user.tmpl`, `synthesizer-user.tmpl`.

## Legacy

`legacy/` holds retired planner text files. Root-level `*.v1.tmpl` copies are deprecated; prefer `pipeline/` + `templates/`.

`atomic-tools/` is no longer part of prompt disclosure or `PromptRegistry`. Native tool schemas are registered in `CapabilityRegistry` and disclosed via mode `tool_pool`; RAG retrieval stays behind `codegen` SDK calls and server-side fallback.
