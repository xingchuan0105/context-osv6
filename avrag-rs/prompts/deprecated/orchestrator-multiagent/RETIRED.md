# Retired: multi-agent orchestrator prompt pack

**Status:** retired from product entry (SaC A2, 2026-07-30).  
**Archived:** 2026-07-31 → `prompts/deprecated/orchestrator-multiagent/`.

## Why

Product chat / RAG / search runs a **single ReAct agent** (`dispatch_agent_mode` + `assemble_mode`).  
There is no production entry that loads task-assigner → worker brief → Answer-phase packs.

## What lived here

| File | Former role |
|------|-------------|
| `orchestrator-base.md` | Task assigner / coordinator system voice |
| `capability-*.dispatch.md` | Brief-writing notes for the assigner only |
| `product-answer-base.md` | Final-answer phase voice after material handoff |
| `answer-from-workspace.md` | Grounding rules when materials = workspace |
| `answer-from-web.md` | Grounding rules when materials = web |
| `answer-dual-source.md` | Dual-source conflict / mix rules |

## Runtime note

`crates/app-chat/src/orchestrator/**` remains as **legacy / optional re-entry** (`run_orchestrator_v1`, tests, `AGENT_ORCHESTRATOR_V2`).  
Those code paths may still `load_system_prompt` files in this directory. They are **not** on the SaC product path.

## Not in this archive (still live)

| Path | Role |
|------|------|
| `prompts/orchestrators/chat-base.md` | Pure chat system voice |
| `prompts/orchestrators/capability-rag.md` | SaC workspace capability voice |
| `prompts/orchestrators/capability-search.md` | SaC web capability voice |
| `prompts/orchestrators/write-refine-system.md` | Write-refine mode (separate product) |

Folder name `orchestrators/` for live files is historical; treat as **system voices**.
