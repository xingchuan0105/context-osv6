Status: extracted from AGENTS/CLAUDE for progressive disclosure. AGENTS.md links here.

# Coding behavior (full text, human reference)

Original long-form behavior essays for humans; the short deltas in `AGENTS.md` win for agents.

## 1. Think Before Coding

**Do not assume. Do not hide confusion. Surface tradeoffs.**

* State your assumptions explicitly before writing code.
* If the user's request is ambiguous or has multiple interpretations, STOP and ask for clarification. Do not silently pick one.
* If a simpler, more standard approach exists that the user didn't mention, suggest it. Push back when warranted.

## 2. Simplicity First (YAGNI)

**Write the absolute minimum code that solves the problem. Nothing speculative.**

* Do NOT add features, abstractions, or "future-proofing" that was not explicitly requested.
* Do NOT add unnecessary error handling for impossible scenarios.
* Do NOT add flexibility or configurability unless asked.
* If your proposed solution is long or complex, rethink and simplify it before outputting.
* Ask yourself: "Would a senior engineer consider this over-engineered?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

* When editing existing code, output ONLY the code block that needs changing, or explicitly describe the surgical modification.
* Do NOT refactor or "improve" adjacent code, comments, or formatting.
* Match the existing code style perfectly, even if you prefer a different standard.
* Do NOT remove pre-existing dead code unless explicitly instructed to do so.
* If your changes create unused imports, variables, or orphaned functions, you MUST remove them.
* **Strict Rule:** Every single line you modify must trace directly back to the user's explicit request.

## 4. Goal-Driven Execution

**Define success criteria. Test-driven verification.**

* Transform vague tasks into verifiable goals.
  * "Add validation" → "Write tests for invalid inputs, then make them pass."
  * "Fix the bug" → "Write a test that reproduces the bug, then fix the code to make it pass."
* For multi-step tasks, outline a brief step-by-step plan before execution:
  `1. [Step 1] -> verify: [check]`
  `2. [Step 2] -> verify: [check]`
* Do not proceed to the next step until the current step's verification criteria are met.

## 5. Architecture Review and Module Design

**Prefer deep modules with small, meaningful interfaces. Avoid shallow pass-through layers.**

* Use these terms consistently when discussing architecture:
  * **Module**: anything with an interface and an implementation.
  * **Interface**: everything a caller must know to use the module correctly, including types, invariants, ordering constraints, error modes, required configuration, and performance characteristics.
  * **Implementation**: the code hidden behind the module interface.
  * **Depth**: leverage at the interface; a deep module hides substantial behavior behind a small interface.
  * **Seam**: the place where behavior can vary without editing callers.
  * **Adapter**: a concrete implementation that satisfies an interface at a seam.
* Apply the deletion test before adding or keeping an abstraction: if deleting the module removes complexity instead of forcing it back into callers, it was probably a shallow pass-through.
* Do not introduce a seam, trait, port, or adapter unless something actually varies across it. One adapter is hypothetical; two justified adapters make the seam real.
* Tests should exercise behavior through the module interface. If a test must reach past the interface into internals, the module shape is probably wrong.
* When doing architecture review or refactoring, read existing domain/context docs and ADRs first if present (`CONTEXT.md`, `CONTEXT-MAP.md`, `docs/adr/`). If absent, proceed without creating them unless the task requires it.

## Code Search (semble)

Use `semble search` to find code by describing what it does or naming a symbol/identifier, instead of grep:

```bash
semble search "authentication flow" ./my-project
semble search "save_pretrained" ./my-project
semble search "save model to disk" ./my-project --top-k 10
```

Use `semble find-related` to discover code similar to a known location (pass `file_path` and `line` from a prior search result):

```bash
semble find-related src/auth.py 42 ./my-project
```

`path` defaults to the current directory when omitted; git URLs are accepted.
If `semble` is not on `$PATH`, use `uvx --from "semble[mcp]" semble` in its place.

### Workflow

1. Start with `semble search` to find relevant chunks.
2. Inspect full files only when the returned chunk is not enough context.
3. Optionally use `semble find-related` with a promising result's `file_path` and `line` to discover related implementations.
4. Use grep only when you need exhaustive literal matches or quick confirmation of an exact string.
