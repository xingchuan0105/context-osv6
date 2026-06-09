# Decision Rules

## When to call `calculator`

Call this tool **only** when the user's request is a single, self-contained numeric expression.

```
Is the user's request a single mathematical expression with only numbers, operators,
functions, and constants?
├── YES → Call calculator
└── NO → Do not call calculator
```

**Use `calculator`**:
- Direct arithmetic: "What is 1583 * 47?"
- Formula evaluation: "Calculate sqrt(144) + pow(2, 5)"
- Trigonometric/logarithmic values: "What is sin(pi / 2)?"
- Intermediate computation in a reasoning chain: the planner needs a precise numeric result for a single expression.

## When NOT to call `calculator`

| Scenario | Why not | Use instead |
|----------|---------|-------------|
| Data analysis, sorting, filtering, DataFrame operations, chart generation | Calculator is expression-only; cannot run code | `code_interpreter` |
| Symbolic math or algebra (solving for x, simplification, derivatives) | Calculator has no symbolic engine | `code_interpreter` or answer from training data |
| Multi-step computation with variables | Calculator does not support variable assignment | `code_interpreter` |
| Unit conversion that requires lookup (currency, historical exchange rates) | Calculator has no unit tables | `web_search` |
| Simple arithmetic the model can do mentally ("2 + 2") | Unnecessary tool latency | Answer directly |

## Interaction with other tools

- `calculator` + `code_interpreter` in the same plan is usually redundant. Prefer `code_interpreter` when computation is part of a larger data-processing script; prefer `calculator` for a single, isolated expression.
- In RAG mode, if the retrieved document contains a formula the user wants evaluated, call `calculator` with the extracted expression.
