# Decision Rules

## When `code_interpreter` is the right tool

- The user asks for data analysis, transformation, or aggregation (sorting, filtering, grouping, statistics).
- The user asks to generate a chart or visualization.
- The user asks for a computation that requires variables, loops, or multi-step logic.
- The user asks to validate, parse, or reformat structured data (JSON, CSV-like lists).
- A retrieved document contains tabular data that needs programmatic processing.

## When to prefer a different tool

- **Simple single-expression math** ("what is 2 + 2", "sin(30°)") → `calculator`. It's faster and has lower overhead.
- **Current weather or forecast** → `weather_query`. The interpreter has no weather API access.
- **Latest news or facts not in training data** → `web_search`. The interpreter cannot access the internet.
- **Retrieving evidence from documents** → RAG tools (`dense_retrieval`, `lexical_retrieval`, etc.). The interpreter cannot query the vector index.

## Interaction with other tools

- `code_interpreter` + `calculator` in the same plan is usually redundant. If the computation is a single expression, use `calculator`; if it needs Python constructs, use `code_interpreter`.
- In Search mode, `code_interpreter` can process web search results (e.g., "extract all prices from these search snippets and compute the average").
- In RAG mode, `code_interpreter` can analyze retrieved chunks (e.g., "count how many times 'antifragility' appears in these chunks").
