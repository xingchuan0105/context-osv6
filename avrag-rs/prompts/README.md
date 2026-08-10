# Prompt layout (CDS) — single-agent product path

LLM-facing prompt assets for the agent, ingestion helpers, and chat postprocess.

**Authoring rules (repo law — see root `AGENTS.md`):**

1. All LLM-facing prose lives under this tree. No hardcoded instruction strings in Rust.
2. **Third-person environment language** (what is true / what is available), not orders (“you must / don’t”).
3. No product/runtime jargon in model-visible bodies unless it is a protocol the model must emit.
4. No realistic-corpus / golden-set entity names in examples.
5. **Context engineering**: high-signal facts about tools, evidence, budgets, and completion semantics — not step pipelines.

## Three-layer synonym map (read this first)

Product and code use different short names for the **same** concepts. This is intentional (stable mode ids vs model-facing product language).

| 模型中文 | 模型/文件英文 | 能力文件 / skill id | 内部 mode / caps | 实现层代号（非模型文案） |
|----------|---------------|---------------------|------------------|--------------------------|
| 知识库 | knowledge base | `capabilities/knowledge-base/contract.md` · skill `knowledge-base` | `mode: rag` · `caps.rag` | sandbox / bridge；loop 文件名可含 `codegen-*` |
| 联网 | web / internet | `capabilities/web/contract.md` · skill `search` | `mode: search` · `caps.search` | `client.web` / `web_search` fallback tool id |
| 文档清单 | docscope | `clusters/docscope/SKILL.md` · skill `docscope` | pipeline 注入 `<docscope_metadata>` | profile 阶段 scope 级聚合 |
| （无） | agent base | `system/agent-base.md` | pure chat + all SaC turns | always first system part |
| 写精修 | write refine | `deprecated/.../write-refine-system.md` | `mode: write_refine` | separate product; not SaC chat tree |

**Loop observation files** named `codegen-*.md` mean “sandbox execution observations”, not the retired skill id `codegen`. Product skill for KB retrieve is **`knowledge-base`**.

## Layout

| Family | Path | How loaded | Role |
|--------|------|------------|------|
| **System** | `system/` | Mode assemble (always first: `agent-base`) | Single-agent main voice |
| **Hints** | `system/hints/` | `include_str!` from assembler / write-core | Small per-round context blocks (`format-hint` / `writing-hint` / `persona-internalize` / `round-counter`) |
| **Capabilities** | `capabilities/<id>/` | Assemble **only when** product mounts that capability | Directory per capability: `contract.md` (short evidence / coverage / cite contract) + `SKILL.md` (method semantics) + `reference/` |
| **Agent guide** | `agent-guide/` | `include_str!` from `app-chat::external_agent_guide` | Standalone API summaries (RAG / search / index / workspace-create) |
| **Clusters** | `clusters/<id>/` | `PromptRegistry` + progressive disclose | Thick world models |
| **Loop** | `loop/` | `agent-loop` `prompt_assets` | Runtime observations |
| **Synthesis** | `synthesis/` | `agent-loop` `prompt_assets` (`synthesis_prompt!`) | Synthesis JSON-envelope contract blocks appended to the synthesis system prompt (P2-2) |
| **Pipeline** | `pipeline/` | Workers / postprocess | Ingestion helpers |
| **Templates** | `templates/` | Pipeline llm calls (`summary-*` / `section-index-*`) | User-turn templates for worker prompts |
| **Deprecated** | `deprecated/**` | Not product SaC entry | Retired monomode, multi-agent, old voices |

## Assembly (SaC)

```text
parts = [ system/agent-base.md ]
if caps.rag (知识库 mounted):   parts += capabilities/knowledge-base/contract.md
if caps.search (联网 mounted):  parts += capabilities/web/contract.md
```

Capability `SKILL.md` files (method semantics) and `reference/` are **not** in `system_prompt_parts`; they are disclosed progressively by `DisclosurePlanner` (mandatory retrieve skills each round + skill_request on demand).

| Product state | System parts |
|---------------|--------------|
| Pure chat | `agent-base` only |
| Knowledge base only | `agent-base` + `capabilities/knowledge-base/contract.md` |
| Web only | `agent-base` + `capabilities/web/contract.md` |
| Dual | `agent-base` + both capability contracts |

Every round also carries the **mandatory memory disclosure** (`clusters/memory/SKILL.md`, derived by `derive_mandatory_retrieve` from CapabilitySet — never listed in mode YAML).

Wired by `app-chat` `assemble_mode` → `AgentRequest.metadata.system_prompt_parts` → agent-loop assembler.

**Host auto_fallback** (`dense_retrieval` / `web_search` tool ids in YAML) is a **server-side** recovery path. It is **not** on the LLM tool schema when SaC clears `tool_pool`; the model still uses the Python sandbox (`client.*`).

## Layering

| Layer | Content |
|-------|---------|
| `agent-base` | Identity, language, final-answer shape, unconditional sandbox base (first-block rule, parallel fan-out, base primitives `history`/`user_profile`/`save`/`load`), memory protocol, pointer to injected modules |
| `capabilities/<id>/contract.md` | Short evidence / coverage / cite contracts when mounted |
| `capabilities/<id>/SKILL.md` | Sandbox method semantics, truncation, tables, method risk |
| `clusters/{docscope, memory, writing, format, brainstorming, verify, index, workspace-create, heavytail-*}` | Thick world models (history / document inventory, answer style, clarify protocol, write-refine metrics, MCP helpers) |
| `loop/*` | What happened this round (budget, sandbox, retrieval_summary) |

## Clusters

| Id | Role |
|----|------|
| `docscope` | Document inventory (scope-level aggregate) + teaching chain `docscope` → `doc_summary` (joint archive: metadata + summary + sections). Injected via `<docscope_metadata>` when requested with `skill_request ["docscope"]` |
| `memory` | History / user profile; on-demand via skill_request (agent-base pointer) |
| `writing` | Answer style layer (v3.1): invariants + max-1 style spoke (`concise` / `professional` / `academic` / `storytelling`) |
| `brainstorming` | Clarify/explore **behavior** protocol (not a writing style spoke) |
| `format` | Output shape (html / slides / outline / teaching) |
| `verify` | Post-synthesis adjudicate skill |
| `heavytail-*` | Write-refine metrics |
| `index` / `workspace-create` | MCP / automation helpers |

## Orchestrator code (architecture note)

Multi-agent **prompts** live under `deprecated/orchestrator-multiagent/`. Product chat entry is **single-agent only** (`dispatch_agent_mode`). Rust `app-chat::orchestrator` was physically deleted on 2026-08-01 (commit `7f2d182d`); no `AGENT_ORCHESTRATOR_V2` re-entry flag exists. See `docs/engineering/2026-07-31-sac-orchestrator-isolation.md`.

## Deprecated

See `deprecated/README.md`. Do not re-wire retired multi-agent packs into product chat.
