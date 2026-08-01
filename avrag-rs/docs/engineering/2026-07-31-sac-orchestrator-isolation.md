# SaC vs orchestrator isolation (2026-07-31)

## Product truth

| Path | Status |
|------|--------|
| `dispatch_agent_mode` → `assemble_mode` → single `ReActLoop` | **Product entry** |
| System prompts: `agent-base` + optional `capabilities/*` | Live |
| Multi-agent task assigner / Answer packs under `prompts/deprecated/orchestrator-multiagent/` | Retired (not product-loaded) |

## What remains in tree

| Component | Why still present |
|-----------|-------------------|
| `crates/app-chat/src/orchestrator/**` | Unit tests, evidence store helpers, optional `run_orchestrator_v1` / `AGENT_ORCHESTRATOR_V2` |
| `modes/orchestrator.yaml` | Marked RETIRED; budget fields only if legacy path enabled |
| `pipeline_steps::run_orchestrator_*` | `#[allow(dead_code)]` / test re-entry — **not** called from product `dispatch_agent_mode` |

## Isolation rules (do not reverse)

1. Do not set product chat entry back to orchestrator without a new ADR.
2. Do not re-add `orchestrator-base` / `product-answer-base` to `assemble_mode` system parts.
3. New model-facing prose for chat/KB/web goes under `prompts/system/`, `prompts/capabilities/`, `prompts/clusters/` only.
4. Prefer deleting orchestrator call sites only after tests that still import the module are migrated or `cfg(test)`-gated in a dedicated wave.

## Feature flag

`AGENT_ORCHESTRATOR_V2=1` (or true/yes/on) may still enable the V2 brain **if** something calls the legacy entry. Default product traffic does not.
