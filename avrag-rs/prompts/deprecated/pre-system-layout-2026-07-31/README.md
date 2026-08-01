# Pre–system/capabilities layout (archived 2026-07-31)

Previous live voices under `prompts/orchestrators/` before single-agent redesign:

| Old path | Replaced by |
|----------|-------------|
| `chat-base.md` | `prompts/system/agent-base.md` (always first) |
| `capability-rag.md` | `prompts/capabilities/knowledge-base.md` (when workspace mounted) |
| `capability-search.md` | `prompts/capabilities/web.md` (when web mounted) |
| `write-refine-system.md` | retained at `prompts/deprecated/pre-system-layout-2026-07-31/write-refine-system.md`, referenced directly by `modes/write_refine.yaml` (write lane, not SaC) |

Assembly rule: **agent-base always**; capabilities only when the product capability set includes them.
