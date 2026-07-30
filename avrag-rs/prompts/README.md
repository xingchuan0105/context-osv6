# Prompt layout (CDS) — SaC product path

LLM-facing prompt assets for the agent, ingestion helpers, and chat postprocess.

**Authoring rules (repo law — see root `AGENTS.md`):**

1. All LLM-facing prose lives under this tree (or is loaded from here). No hardcoded instruction strings in Rust.
2. Write in **third-person environment language** (what is true / what happened), not orders (“you must / don’t”).
3. Avoid product/runtime jargon in model-visible bodies: no host hard-gates, state-machine names, ADR ids, or internal type names unless the string **is** a protocol the model must emit (then define it in plain words once).
4. No realistic-corpus / golden-set entity names in examples.
5. Prefer **context engineering**: high-signal facts about tools, evidence, budgets, and completion semantics — not step-by-step pipelines.

## Layout

| Family | Path | How loaded | Typical use |
|--------|------|------------|-------------|
| **A — Clusters** | `clusters/<id>/` | `PromptRegistry` (skill id + optional `reference/`) | Progressive skills: retrieval, memory, writing, format, … |
| **B — System voices** | `orchestrators/` *(name historical)* | Mode assemble | SaC capability manuals + chat base + write-refine |
| **C — Loop observations** | `loop/` | `agent-loop` `prompt_assets` (`include_str!`) | Runtime facts injected into the message list |
| **D — Pipeline** | `pipeline/*.system*.md` | Worker / chat postprocess `include_str!` | Ingestion summary, TOC, triplets, profile delta |
| **Templates** | `templates/*.tmpl` | Paired user templates | User-side shells for pipeline jobs |
| **Deprecated** | `deprecated/**` | Not product SaC entry; legacy tests may still load | Retired monomode / synthesis / multi-agent orchestrator |

## Product modes (current — single agent)

| Mode | System voice | Retrieve skill | Notes |
|------|--------------|----------------|-------|
| Pure chat | `orchestrators/chat-base.md` | — | Utility tools only |
| Workspace RAG | `orchestrators/capability-rag.md` | `clusters/codegen` (+ tables ref) | In-loop answer; `SELECTED: #n` |
| Web search | `orchestrators/capability-search.md` | `clusters/search` | `[[web:n]]` |
| Dual | both capability manuals | codegen + search | Same single ReAct loop |
| Write refine | `orchestrators/write-refine-system.md` | heavytail-* | Separate mode, not SaC chat |

**Retired (not product entry):** multi-agent task assigner + Answer-phase packs → `deprecated/orchestrator-multiagent/`.

## Clusters (A)

| Id | Role |
|----|------|
| `codegen` | Workspace retrieve via Python sandbox |
| `search` | Web retrieve via Python sandbox |
| `memory` | Longer history / profile |
| `metadata` | Workspace document inventory |
| `writing` / `format` | Final-answer style / shape (at most one reference each) |
| `heavytail-*` | Writing fingerprint metrics / refine decisions |
| `index` / `workspace-create` | MCP / automation ingest helpers |

## Loop (C)

See `loop/README.md`. Bodies are **observations** of runtime state.

## Deprecated

See `deprecated/README.md`. Not scanned for product skill disclosure; do not re-wire into chat/rag/search YAML.
