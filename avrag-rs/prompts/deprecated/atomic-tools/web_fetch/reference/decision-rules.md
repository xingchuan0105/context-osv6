# Decision Rules

## Planner Decision Tree

Use this flow to decide whether to call `web_fetch`.

```
Does the user provide a specific URL?
├── YES → Is the request about that URL's content?
│   ├── YES → Call web_fetch
│   └── NO  → Do not call (URL is incidental)
└── NO → Does the user ask about a specific web page by name/description?
    ├── YES → Could the information already be in conversation context?
    │   ├── YES → Do not call; answer from context
    │   └── NO  → Call web_search first, then web_fetch on the best result
    └── NO → Do not call web_fetch
```

## When to call

1. **URL explicitly provided + content needed**
   - "What does https://blog.rust-lang.org/2025/01/09/Rust-1.85.0.html say?"
   - "Summarize this article: https://..."

2. **Specific page referenced by name, after confirming it's not in context**
   - "According to the Wikipedia page on Rust, what are its key features?"
   - Check context first; if absent, search then fetch.

3. **Deep-dive on a web_search result**
   - After `web_search` returns results, call `web_fetch` on the single most relevant URL for full content.

## When NOT to call

1. **No URL and no specific page named**
   - "What is Rust?" → Use `web_search`, not `web_fetch`.

2. **Real-time or rapidly changing data**
   - Stock prices, weather, live scores → Use specialized tools or `web_search`.

3. **Information already in conversation context**
   - Prefer existing context. Do not re-fetch content the agent already has.

4. **RAG mode**
   - This tool is not available in RAG mode. Use built-in retrieval tools (`dense_retrieval`, `lexical_retrieval`, etc.) instead.

## Parameter selection

- `url`: Always pass the exact URL. Do not guess or construct URLs.
- `max_length`: Default to 8000. Increase to 16000 or 32000 only when the user explicitly asks for detailed reading of a long document.
