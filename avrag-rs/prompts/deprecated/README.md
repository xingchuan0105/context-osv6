# Deprecated prompts (not product-loaded by default)

Bodies here are **not** scanned by `agent-tools/build.rs` skill registration for product SaC.  
Some legacy code paths may still `include_str!` / `load_system_prompt` specific files for tests or optional orchestrator re-entry.

| Subdir | Content |
|--------|---------|
| `monomode-system/` | Pre–capability-manual monomode system prompts |
| `synthesis/` | Old mandatory JSON synthesis answer skills |
| `orchestrator-multiagent/` | Task-assigner + Answer-phase packs (retired with SaC) |
| `loop-legacy/` | Host no-chunk continue/grace observations (skill-owned grounding era) |
| `pre-system-layout-2026-07-31/` | chat-base / capability-rag|search before `system/` + `capabilities/` |

Live product layout: `prompts/system/agent-base.md` + optional `prompts/capabilities/{knowledge-base,web}.md`.
