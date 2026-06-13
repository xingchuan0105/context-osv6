# ReAct Loop State Machine

The agent retrieve loop (`ReActLoop::run`) is a finite-state process over **iteration rounds**.
Each round discloses skills, calls the LLM, optionally executes tools, then either continues,
breaks to synthesis, or returns a direct answer.

## States

| State | Entry | Exit transitions |
|-------|-------|------------------|
| **NormalizeQuery** | `run()` starts | Clarify → terminal `Clarified`; else → **RetrieveRound** |
| **RetrieveRound** | `iteration < max_iterations` | Budget exhausted → **EvaluateExit**; iteration outcome → see below |
| **EvaluateExit** | budget exhausted or break signal | `LoopPolicy::synthesis_gate` → **Synthesize** or **Degrade** |
| **Synthesize** | gate allows synthesis | **FinishRun** with composed answer |
| **Degrade** | no evidence / give-up gate | **FinishRun** with degraded answer |
| **DirectAnswer** | LLM emits final prose in-loop | **FinishRun** (skips synthesis when configured) |
| **FinishRun** | terminal | emit citations, persist, return `AgentRunResult` |

## RetrieveRound sub-transitions (`run_iteration`)

```
RetrieveRound
  ├─ assemble context (DisclosurePlanner + ContextAssembler)
  ├─ LLM call (retrieve phase)
  ├─ parse output
  │    ├─ tool calls → dispatch tools → Continue (next iteration)
  │    ├─ skill request → validate → Continue
  │    ├─ direct answer prose → DirectAnswer
  │    └─ empty / blocked early-stop → BreakToSynthesis
  └─ optimizer hints (duplicate chunks, budget warning) → Continue
```

## Iteration module layout (`loop/iteration/`)

`run_iteration` and its dispatchers are split across:

| File | Responsibility |
|------|---------------|
| `iteration/mod.rs` | `run_iteration` + `apply_llm_output` orchestration |
| `iteration/state.rs` | `IterationState`, `IterationControl`, `IterationOutcome`, `disclosed_skill_ids` |
| `iteration/assemble.rs` | `assemble_retrieve_context` + `call_retrieve_llm` |
| `iteration/content_dispatch.rs` | `dispatch_content` (direct-answer / skill-request / blocked branches) + `iteration_llm_usage` |
| `iteration/tests.rs` | per-iteration outcome tests (native tool, codegen, sandbox break, content branches) |

Sibling files at `loop/` (not yet folded into `iteration/`):
`iteration_tools.rs` — `dispatch_native_tool_calls`; `iteration_codegen.rs` — `dispatch_codegen`.
See Brooks-Lint review 2026-06-13 for the conceptual-integrity note on this asymmetry.

## Policy seam (`loop/policy/`)

Loop behaviour is configured through **`LoopPolicy`** (≤3 public methods):

1. `load_mode` — YAML mode config (`policy::config` submodule)
2. `synthesis_gate` — post-loop evidence / budget gate (`policy::exit_policy` submodule)
3. `plan_retrieve` — progressive disclosure slices (`policy::disclosure_plan` submodule)

Callers outside `policy/` should prefer `LoopPolicy`; submodule paths remain for in-crate tests
and gradual migration.

### `policy/config/` layout

| File | Responsibility |
|------|---------------|
| `policy/config/mod.rs` | re-export facade |
| `policy/config/config_types.rs` | `ModeConfig`, `LoopExitConfig`, `BudgetConfig`, `AutoFallbackConfig`, etc. |
| `policy/config/mode_loader.rs` | `load_mode_config`, `load_system_prompt`, `loop_exit_for_mode`, validation |
| `policy/config/skill_catalog.rs` | `SkillCatalogConfig`, `SkillCluster`, `DiscloseAt`, custom deserializer |
| `policy/config/tests.rs` | mode YAML deserialization + tier budget tests |

## Invariants

- `base_message_count` messages (history + user query) are never truncated.
- Evidence tools are mode-specific (`exit_policy` constants).
- Cancellation is checked at the top of each iteration.
