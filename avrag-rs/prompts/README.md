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
| 知识库 | knowledge base | `capabilities/knowledge-base.md` · skill `knowledge-base` | `mode: rag` · `caps.rag` | sandbox / bridge；loop 文件名可含 `codegen-*` |
| 联网 | web / internet | `capabilities/web.md` · skill `search` | `mode: search` · `caps.search` | `client.web` / `web_search` fallback tool id |
| （无） | agent base | `system/agent-base.md` | pure chat + all SaC turns | always first system part |
| 写精修 | write refine | `deprecated/.../write-refine-system.md` | `mode: write_refine` | separate product; not SaC chat tree |

**Loop observation files** named `codegen-*.md` mean “sandbox execution observations”, not the retired skill id `codegen`. Product skill for KB retrieve is **`knowledge-base`**.

## Layout

| Family | Path | How loaded | Role |
|--------|------|------------|------|
| **System** | `system/` | Mode assemble (always first: `agent-base`) | Single-agent main voice |
| **Capabilities** | `capabilities/` | Assemble **only when** product mounts that capability | Short task contracts (知识库 / 联网) |
| **Clusters** | `clusters/<id>/` | `PromptRegistry` + progressive disclose | Thick world models |
| **Loop** | `loop/` | `agent-loop` `prompt_assets` | Runtime observations |
| **Pipeline** | `pipeline/` | Workers / postprocess | Ingestion helpers |
| **Deprecated** | `deprecated/**` | Not product SaC entry | Retired monomode, multi-agent, old voices |

## Assembly (SaC)

```text
parts = [ system/agent-base.md ]
if caps.rag (知识库 mounted):   parts += capabilities/knowledge-base.md
if caps.search (联网 mounted):  parts += capabilities/web.md
```

| Product state | System parts |
|---------------|--------------|
| Pure chat | `agent-base` only |
| Knowledge base only | `agent-base` + `capabilities/knowledge-base` |
| Web only | `agent-base` + `capabilities/web` |
| Dual | `agent-base` + both capability contracts |

Wired by `app-chat` `assemble_mode` → `AgentRequest.metadata.system_prompt_parts` → agent-loop assembler.

**Host auto_fallback** (`dense_retrieval` / `web_search` tool ids in YAML) is a **server-side** recovery path. It is **not** on the LLM tool schema when SaC clears `tool_pool`; the model still uses the Python sandbox (`client.*`).

## Layering

| Layer | Content |
|-------|---------|
| `agent-base` | Identity, language, final-answer shape, unmounted boundary, memory protocol, pointer to injected modules |
| `capabilities/*` | Short evidence / coverage / cite contracts when mounted |
| `clusters/knowledge-base`, `search`, … | Sandbox APIs, truncation, tables, method risk |
| `loop/*` | What happened this round (budget, sandbox, retrieval_summary) |

## Clusters

| Id | Role |
|----|------|
| `knowledge-base` | Knowledge-base retrieve via Python sandbox (v4.1+: gotchas, multi-claim checklist, table contrast examples). Skill-reg subset: `scripts/sac-skill-fail6-reg.sh` + `docs/engineering/2026-07-31-sac-skill-fail6-reg.md` |
| `search` | Web (联网) retrieve via Python sandbox |
| `memory` / `metadata` | History / document inventory |
| `writing` / `format` | Answer style / shape (also on retrieve when request has writing/format hints — SaC ProseOnly path) |
| `heavytail-*` | Write-refine metrics |
| `index` / `workspace-create` | MCP / automation helpers |

## Orchestrator code (architecture note)

Multi-agent **prompts** live under `deprecated/orchestrator-multiagent/`. Product chat entry is **single-agent only** (`dispatch_agent_mode`). Rust `app-chat::orchestrator` remains for tests / optional `AGENT_ORCHESTRATOR_V2` re-entry — not the default product path. See `docs/engineering/2026-07-31-sac-orchestrator-isolation.md`.

## Deprecated

See `deprecated/README.md`. Do not re-wire retired multi-agent packs into product chat.
