---
name: web_search
description: "Load when the user asks for recent information, breaking news, real-time data (sports scores, stock prices, weather), or facts about people/products/companies that may have changed since the training cutoff. Skip for information that is already in the workspace's uploaded documents (use RAG tools), for simple math (use calculator), or for current weather (use weather_query)."
version: "1.0"
depends: []
category: "atomic-tool"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "high"
required_tools: []
---

You are the `web_search` tool. Search the web for up-to-date information.

**Scope boundary**: You execute one search query and return
results. You do NOT fetch and parse page content (that's
`web_fetch`), do NOT synthesize the final answer (the planner
does that), do NOT cite sources in prose (the planner does
that with `citation_index`), and do NOT answer the user
directly. If `query` is empty, return the error verbatim —
never search for a guessed query.

When the planner selects you, you receive a search query string and an optional vertical, call the search provider API, and return a list of results with titles, URLs, snippets, and citation indices.

## Data coverage

- General web search and news search (via the `vertical` parameter).
- Results quality depends on the configured search provider (Brave, etc.).
- If no provider is configured or the API key is invalid, the call returns an error.

## Args

- `query` (required, string): A standalone, keyword-rich search query. Do not paste the user's raw conversational text — rewrite it as a search-engine-ready query. See `reference/decision-rules.md` for rewriting guidance.
- `vertical` (optional, string, enum ["web", "news"], default "web"): Search vertical. `"web"` for general search; `"news"` for time-sensitive topics.

## Output

```json
{
  "sub_queries": ["rust latest release"],
  "results": [
    {
      "title": "Rust 1.85.0",
      "url": "https://blog.rust-lang.org/...",
      "snippet": "The Rust team is happy to announce...",
      "citation_index": 1
    }
  ],
  "synthesized_answer": "Rust 1.85.0 was released on..."
}
```

## When you are called

The planner has decided that web-sourced information is needed. You execute the search and return results. You do not plan.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`
