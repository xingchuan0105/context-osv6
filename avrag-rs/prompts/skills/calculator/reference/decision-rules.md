# Decision Rules

## When `calculator` is the right tool

- The user asks a direct mathematical question: "What is 1583 * 47?"
- The user needs a precise numeric result from a formula.
- The user asks for trigonometric, logarithmic, or statistical values.
- A multi-step reasoning chain needs an intermediate numeric computation.

## When to prefer a different tool

- **Data analysis or transformation** (sorting, filtering, DataFrame ops, chart generation) → `code_interpreter`. The calculator is expression-only; it cannot run Python.
- **Symbolic math or algebra** (solving for x, simplification) → `code_interpreter` or answer directly from training data.
- **Unit conversion that requires lookup** (currency, historical exchange rates) → `web_search`. The calculator has no unit tables.
- **Simple arithmetic the model can do mentally** ("2 + 2") → no tool needed; answer directly to save latency.

## Interaction with other tools

- `calculator` + `code_interpreter` in the same plan is usually redundant. Prefer `code_interpreter` when the computation is part of a larger data-processing script; prefer `calculator` for a single, isolated expression.
- In RAG mode, if the retrieved document contains a formula the user wants evaluated, call `calculator` with the extracted expression.
