# Extending the Agent Loop platform

Owner crate: `agent-loop` (ReAct + policy). Tools live in `agent-tools`.
Orchestration (sessions, pipeline, UnifiedAgent shell) stays in `app-chat`.

## Boundaries (do not violate)

| Concern | Where | Forbidden |
|---------|-------|-----------|
| Tool **execute** | `agent_tools::ToolCatalog` + `dispatch_tool` only | New match arms in loop / atomic_tools / dual HashMap |
| Mode behavior | YAML `ModeConfig` (`tool_pool`, skills, budgets) | Hard-coding mode branches inside iteration |
| Capability / Skill / Tool names | ADR-0006 §5a product layers | Merging registries into one “everything map” |
| Untrusted tool/obs text | `agent_loop::untrusted_input` | Ad-hoc scrubbers in app-chat |
| Chat product shell | `app-chat` (pipeline, persistence, SSE glue) | Pulling session/HTTP into agent-loop |

## Extension recipes

### 1. New tool

1. Implement + register in `agent-tools` catalog.
2. Add tool id to the mode’s `tool_pool` (and skill disclosure if progressive).
3. Call path must be `dispatch_tool` only — no loop-local execute.

### 2. New mode / mode knobs

1. Add or edit `modes/*.yaml` (`ModeConfig`).
2. Prefer skill catalog + disclosure plan over new Rust control flow.
3. Budget / exit policy: extend `LoopPolicy` config, not ad-hoc `break`s in iteration.

### 3. Prompt / context transforms

- Prefer `LoopHooks` (`transform_context` / `convert_to_llm`) over forking `ReActLoop::run`.
- Iteration budget injection and disclosure assembly already live in assembler / policy.

### 4. Product orchestration (sessions, write, billing)

- Stay in `app-chat` / domain crates.
- Write mode is intentionally **not** UnifiedAgent — see `app-chat` writer boundary.

## Verification

```bash
cd avrag-rs
cargo test -p agent-tools --lib
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
```

State machine detail: `src/react_loop/STATE_MACHINE.md`.
