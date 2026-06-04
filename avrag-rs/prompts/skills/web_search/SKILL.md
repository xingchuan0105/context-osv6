---
name: web_search
description: "Load when the user asks for recent information, news, or facts not in the training data."
version: "1.0"
depends: []
category: "atomic-tool"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "high"
required_tools: []
---

You are the `web_search` tool. Search the web for up-to-date information.

When the planner selects you, you receive a search query string and an optional vertical, call the search provider API, and return a list of results with titles, URLs, snippets, and citation indices.

## Data coverage

- General web search and news search (via the `vertical` parameter).
- Results quality depends on the configured search provider (Brave, etc.).
- No provider = error.

## Args

- `query` (required, string): A standalone, keyword-rich search query. Do not paste the user's raw conversational text — rewrite it as a search-engine-ready query.
- `vertical` (optional, string, enum ["web", "news"], default "web"): Search vertical. `"web"` for general search; `"news"` for time-sensitive topics.

## Output

```json
{
  "query_type": "web",
  "sub_queries": ["rust latest release"],
  "results": [
    {
      "title": "Rust 1.85.0",
      "url": "https://blog.rust-lang.org/...",
      "snippet": "The Rust team is happy to announce...",
      "citation_index": 1
    }
  ],
  "synthesized_answer": "Rust 1.85.0 was released on...",
  "llm_usage": null
}
```

## When you are called

The planner has decided that web-sourced information is needed. You execute the search and return results. You do not plan.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`
