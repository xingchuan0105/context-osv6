# Extending the Agent Loop platform

Owner crate: `agent-loop` (ReAct + policy). Tools live in `agent-tools`.
Orchestration (sessions, pipeline, UnifiedAgent shell) stays in `app-chat`.

Architecture plan: `docs/plans/2026-07-29-pi-informed-agent-architecture-optimization.md`.

## Vocabulary (do not merge)

| Term | Meaning | Forbidden |
|------|---------|-----------|
| **Tool** | Executable via `ToolCatalog` / `dispatch_tool` | Ad-hoc execute match arms in the loop |
| **SkillMd** | Prompt-only `SKILL.md` (`progressive::Skill`) | Registering MD as executable |
| **SkillComponent** | Legacy name for executable builtins (is a **Tool**) | Treating as “prompt skill” |
| **Capability** | Mode metadata + **`PolicyEnforcer`** (policy truth) | Collapsing into one map with tools |
| **HostTool** | Orchestrator intercepts (`delegate_*`, `finish_answer`, `evidence_fetch`) | Registering into `ToolCatalog` |
| **LoopHooks** | Context transforms (`transform_context` / `convert_to_llm`) | **Second policy engine** (allowlists/denies) |

## Boundaries (do not violate)

| Concern | Where | Forbidden |
|---------|-------|-----------|
| Tool **execute** | `agent_tools::ToolCatalog` + `dispatch_tool` only | New match arms in loop / dual HashMap |
| Tool **policy** (allow/deny/tier) | `PolicyEnforcer` + catalog metadata | Parallel deny rules inside `LoopHooks` |
| Mode behavior | YAML `ModeConfig` (`tool_pool`, skills, budgets) | Hard-coding mode branches inside iteration |
| Capability / Skill / Tool names | ADR-0006 §5a product layers (T4) | Merging registries into one “everything map” |
| Untrusted tool/obs text | `agent_loop::untrusted_input` | Ad-hoc scrubbers in app-chat |
| Chat product shell | `app-chat` (pipeline, persistence, SSE glue) | Pulling session/HTTP into agent-loop |
| Write tools | Write lane / `write_refine` only | ReAct `ToolCatalog` (T2) |

## Dual loops (product shape)

```text
User turn
  └─ Orchestrator brain (HostTools: delegate_*/evidence_fetch/finish_answer)
        │  not in ToolCatalog; not ReActLoop
        ├─ WorkerSession (channel-persistent)
        │     └─ ReActLoop::run[_with_hooks]
        │           ├─ Assembler + SkillMd disclosure
        │           ├─ codegen (RAG main path) / dispatch_tool
        │           └─ LoopHooks::transform_context (windowing only)
        └─ chat_exit / evidence hydrate → user answer
```

- **Do not** force the brain onto `ReActLoop` (plan C4a).
- Shared fragments only when proven (budget helpers, message format) — plan C4b.

## Policy vs hooks

| Concern | Truth source | Hooks may |
|---------|--------------|-----------|
| Permission / tier / risk | **`PolicyEnforcer`** (inside `dispatch_tool`) | `before_tool_call` default **never blocks**; tests/host only |
| Message window / prefix cache | **`LoopHooks::transform_context`** | Own this |
| LLM message shape at API boundary | **`LoopHooks::convert_to_llm`** | Own this (default identity) |
| Tool / codegen bridge finish | (events + progress) | `after_tool_call` observe |
| Per-iteration end | `IterationControl` | `on_turn_end` observe |
| Exit gates (evidence/budget) | `LoopPolicy` / `exit_policy` | Observe only |

Default product path: `ReActLoop::run` → `StandardLoopHooks` (high watermark 32, low 20).
Custom transforms: `ReActLoop::run_with_hooks(..., &my_hooks)`.

**Do not** implement tier/risk allowlists inside hooks (plan D7).

## Runtime deps (Wave B1 + follow-up)

- Side-effect runtimes live in `LoopRuntimeDeps` (`rag_runtime`, `search_executor`,
  `chat_persistence`, `code_interpreter`), not as loose fields on `ReActLoop`.
- Builders: `with_rag_runtime` / `with_search_executor` / `with_chat_persistence` /
  `with_runtime_deps`.
- **CodegenPort:** `LoopRuntimeDeps::execute_codegen_bridged_with_session` owns
  `RuntimeBridge`; loop files use [`BridgeCallObs`] only. Grep: `avrag_rag_core::`
  under `react_loop/` should only appear in `deps.rs` (+ optional public builder
  signatures on `ReActLoop`).

## Product contract (Wave C)

- Facade: `agent_loop::product_contract` (answer + handoff compiler).
- Stable paths still work: `answer_contract`, `output_compiler`.
- Worker ↔ loop metadata: `worker_contract::RETRIEVAL_ALIAS_START_METADATA`
  (app-chat `ALIAS_START_METADATA` aliases the same string).
- HostTools: `app_chat::orchestrator::HOST_TOOL_NAMES` — never on `ToolCatalog`
  (**C4a:** do not force brain onto `ReActLoop`; **C4b:** only extract proven shared helpers).

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

- Prefer `LoopHooks` + `run_with_hooks` over forking `ReActLoop::run`.
- Iteration budget injection and disclosure assembly live in assembler / policy.
- `StandardLoopHooks`: append-only until `base + compact_high_watermark`, then pair-safe compact to `max_react_messages`.

### 4. Product orchestration (sessions, write, billing)

- Stay in `app-chat` / domain crates.
- Write mode is intentionally **not** UnifiedAgent — see `app-chat` writer boundary.
- HostTools stay host-intercepted in orchestrator.

### 5. Steering / follow-up queues

- `react_loop::message_queue::LoopMessageQueue` is a **deprecated placeholder** (Wave A2).
- SaaS one-shot turns do not inject mid-loop user messages. Do not wire or delete without product/ADR decision.

## Verification

```bash
cd avrag-rs
cargo test -p agent-tools --lib
cargo test -p agent-loop --lib
cargo test -p app-chat --lib
```

State machine detail: `src/react_loop/STATE_MACHINE.md`.
