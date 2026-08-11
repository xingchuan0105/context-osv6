# Prompt layout (CDS) — Lead + Workers product path

LLM-facing prompt assets for the agent, ingestion helpers, and chat postprocess.
Product rag/search/dual retrieve = **Lead + channel Workers**; pure chat remains single-agent SaC.

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
| （无） | agent base | `system/agent-base.md` | pure chat + session identity | often first system part |
| Lead | lead | `system/lead-base.md` · `clusters/lead/SKILL.md` | rag/search/dual plan+synth | assembly when caps mounted |
| RAG Worker | rag worker | `system/worker-sandbox.md` · `workers/rag/SKILL.md` | nested short SaC | Worker system parts only |
| Web Worker | web worker | `workers/web/SKILL.md` | host multi-query leaf | skill + host pack |
| 写精修 | write refine | `deprecated/.../write-refine-system.md` | `mode: write_refine` | separate product; not SaC chat tree |

**Loop observation files** named `codegen-*.md` mean “sandbox execution observations”, not the retired skill id `codegen`. Product skill for KB retrieve is **`knowledge-base`**.

## Layout

| Family | Path | How loaded | Role |
|--------|------|------------|------|
| **System** | `system/` | Mode assemble (`agent-base`, `lead-base`) / Worker nest (`worker-sandbox`) | Session + Lead + Worker voice |
| **Hints** | `system/hints/` | `include_str!` from assembler / write-core | Small per-round context blocks |
| **Capabilities** | `capabilities/<id>/` | Assemble when product mounts that capability | `contract.md` + `SKILL.md` + `reference/` |
| **Workers** | `workers/{rag,web}/` + `workers/default-*.md` | Nested RAG SaC / brief defaults | Channel Worker skills |
| **Agent guide** | `agent-guide/` | `include_str!` from `app-chat::external_agent_guide` | Standalone API summaries |
| **Clusters** | `clusters/<id>/` | `PromptRegistry` + progressive disclose | Thick world models (`lead`, memory, writing, …) |
| **Loop** | `loop/` | `agent-loop` `prompt_assets` | Runtime observations (must register tags in `host_markers`) |
| **Synthesis** | `synthesis/` | `agent-loop` `prompt_assets` | Synthesis contract blocks |
| **Pipeline** | `pipeline/` | Lead plan + ingestion helpers | `lead-plan.*`, summary templates |
| **Templates** | `templates/` | Pipeline llm calls | User-turn templates for worker prompts |
| **Deprecated** | `deprecated/**` | Not product entry | Retired monomode / multi-agent |

## Assembly (product)

```text
# Pure chat
parts = [ system/agent-base.md ]

# rag / search / dual (Lead+Workers)
parts = [ system/agent-base.md, system/lead-base.md ]
if caps.rag:     parts += capabilities/knowledge-base/contract.md
if caps.search:  parts += capabilities/web/contract.md

# Nested RAG Worker short SaC (not product session parts)
worker_parts = [ system/worker-sandbox.md, knowledge-base/contract.md, workers/rag/SKILL.md ]
```

Capability method manuals and `reference/` are progressive via `DisclosurePlanner` (mandatory retrieve + skill_request).

| Product state | Session system parts |
|---------------|----------------------|
| Pure chat | `agent-base` |
| Knowledge base only | `agent-base` + `lead-base` + KB contract |
| Web only | `agent-base` + `lead-base` + web contract |
| Dual | `agent-base` + `lead-base` + both contracts |

Wired by `app-chat` `assemble_mode` → `AgentRequest.metadata.system_prompt_parts` → agent-loop assembler.

**Host leaves** (Web multi-query search+CRW, optional host lexical re-brief, BASE weather/calculator) are **not** LLM tool schemas. Nested RAG short SaC still uses Python sandbox (`client.*`).

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
