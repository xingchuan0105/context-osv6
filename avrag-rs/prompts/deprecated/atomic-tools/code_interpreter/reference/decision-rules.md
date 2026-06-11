# Decision Rules

## When to call `code_interpreter`

Use this tool when the task requires **multi-step Python logic** that goes beyond a single arithmetic expression.

```
Does the task require running Python code?
├── NO → Do not call code_interpreter
└── YES → Is it a single arithmetic expression with no variables/loops?
    ├── YES → Use `calculator` instead (faster, lower overhead)
    └── NO → Does it require network access, file system, or system commands?
        ├── YES → Do not call code_interpreter; use other tools
        └── NO → Call code_interpreter
```

**Call `code_interpreter`**:
- Data analysis, transformation, or aggregation (sorting, filtering, grouping, statistics).
- Chart or visualization generation.
- Computation that requires variables, loops, or multi-step logic.
- Validate, parse, or reformat structured data (JSON, CSV-like lists).
- A retrieved document contains tabular data that needs programmatic processing.

## When NOT to call `code_interpreter`

| Scenario | Why not | Use instead |
|----------|---------|-------------|
| Simple single-expression math ("what is 2 + 2", "sin(30°)") | Overhead too high for trivial computation | `calculator` |
| Current weather or forecast | No network access | `weather_query` |
| Latest news or facts not in training data | No internet access | `web_search` |
| Retrieving evidence from documents | Cannot query vector index | RAG tools (`dense_retrieval`, `lexical_retrieval`) |
| File system operations, network requests, process spawning | Blocked by sandbox | Other tools or inform user of limitation |
| Web scraping or system automation | Not a general-purpose shell | Inform user this is out of scope |

## Interaction with other tools

- `code_interpreter` + `calculator` in the same plan is usually redundant. If the computation is a single expression, use `calculator`; if it needs Python constructs, use `code_interpreter`.
- In Search mode, `code_interpreter` can process web search results (e.g., "extract all prices from these search snippets and compute the average").
- In RAG mode, `code_interpreter` can analyze retrieved chunks (e.g., "count how many times 'antifragility' appears in these chunks").
