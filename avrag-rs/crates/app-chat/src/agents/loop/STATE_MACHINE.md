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

## Policy seam (`loop/policy/`)

Loop behaviour is configured through **`LoopPolicy`** (≤3 public methods):

1. `load_mode` — YAML mode config (`config` submodule)
2. `synthesis_gate` — post-loop evidence / budget gate (`exit_policy` submodule)
3. `plan_retrieve` — progressive disclosure slices (`disclosure_plan` submodule)

Callers outside `policy/` should prefer `LoopPolicy`; submodule paths remain for in-crate tests
and gradual migration.

## Invariants

- `base_message_count` messages (history + user query) are never truncated.
- Evidence tools are mode-specific (`exit_policy` constants).
- Cancellation is checked at the top of each iteration.
