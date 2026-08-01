# SaC vs orchestrator isolation (2026-07-31, updated 2026-08-01)

## Product truth

| Path | Status |
|------|--------|
| `dispatch_agent_mode` → `assemble_mode` → single `ReActLoop` | **Product entry** |
| System prompts: `agent-base` + optional `capabilities/*` | Live |
| Multi-agent task assigner / Answer packs under `prompts/deprecated/orchestrator-multiagent/` | Retired (not product-loaded) |

## What remains in tree (2026-08-01: orchestrator physically deleted)

| Component | Status |
|-----------|--------|
| `crates/app-chat/src/orchestrator/**` (9.2k LOC) | **Deleted 2026-08-01** (commit: orchestrator removal wave) |
| `modes/orchestrator.yaml` | **Deleted** |
| `pipeline_steps::run_orchestrator_*` / `run_orchestrator_v1` | **Deleted** (product entry was already `dispatch_agent_mode` → `run_general_mode`) |
| `prompts/plan.rs` + `prompts/tests/plan.rs` (RAG PLAN phase) | **Deleted** (orchestrator-only) |
| `UnifiedAgentService::with_orchestrator_llm` | **Deleted** (no callers) |
| `prompts/deprecated/orchestrator-multiagent/` | Kept — archived prompts; `guardrails/prompt_leak.rs` still fingerprints them |
| `WriterOrchestrator` (writer/mod.rs) | Kept — unrelated (write-lane refinement loop) |
| `orchestrator_context::OrchestratorContext` | Kept — unrelated (auth/storage/billing aggregate) |

`AGENT_ORCHESTRATOR_V2` feature flag is **gone** with the module; no re-entry path exists.

## Isolation rules (do not reverse)

1. Do not set product chat entry back to orchestrator without a new ADR.
2. Do not re-add `orchestrator-base` / `product-answer-base` to `assemble_mode` system parts.
3. New model-facing prose for chat/KB/web goes under `prompts/system/`, `prompts/capabilities/`, `prompts/clusters/` only.
4. Orchestrator code is deleted, not `cfg(test)`-gated — reintroducing it requires a new ADR plus re-importing the archived prompts.
