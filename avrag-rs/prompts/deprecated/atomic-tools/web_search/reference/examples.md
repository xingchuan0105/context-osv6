# Examples

## General web search

```json
{ "query": "Rust latest release version features", "vertical": "web" }
```
Result:
```json
{
  "sub_queries": ["Rust latest release version features"],
  "results": [
    {
      "title": "Announcing Rust 1.85.0",
      "url": "https://blog.rust-lang.org/2025/02/...",
      "snippet": "The Rust team is happy to announce a new version of Rust, 1.85.0...",
      "citation_index": 1
    }
  ],
  "synthesized_answer": "Rust 1.85.0 was released in early 2026..."
}
```

## News search

```json
{ "query": "OpenAI GPT-5 announcement", "vertical": "news" }
```
Result: News articles about recent OpenAI announcements, with `citation_index` values.

## Simple factual query

```json
{ "query": "capital of France" }
```
Result: Results confirming Paris is the capital of France.

Note: This is a borderline case — "capital of France" is
answerable from training data with high confidence and may not
need `web_search` at all. In practice, prefer answering such
queries directly. Use `web_search` only when the user
explicitly asks for sources or when freshness matters.

## Error: missing query

```json
{}
```
Result:
```json
{
  "status": "error",
  "error": {
    "code": "MISSING_QUERY",
    "message": "missing query"
  }
}
```

## Error: no provider configured

```json
{ "query": "test" }
```
When no search provider is configured:
Result:
```json
{
  "status": "error",
  "error": {
    "code": "NO_PROVIDER_CONFIGURED",
    "message": "search provider not available"
  }
}
```

## Query rewriting examples

### Stripping conversational filler

User asks: "Hey, can you find out what the weather was like during the Tokyo Olympics in 2021?"

**Good** (keyword-rich, standalone):
```json
{ "query": "Tokyo Olympics 2021 weather conditions summer" }
```

**Bad** (conversational filler, will hurt recall):
```json
{ "query": "Hey, can you find out what the weather was like during the Tokyo Olympics in 2021?" }
```

### Adding specificity

User asks: "What is the latest version of Rust?"

**Good**: `{ "query": "Rust latest release version" }`
**Bad**:  `{ "query": "latest rust" }` (too broad) or
           `{ "query": "rust" }` (single word, low signal)

## Multi-query parallel search (Search mode)

**Context**: User asks "compare React, Vue, and Svelte in 2026."
The Search strategy decomposes into three sub-queries and issues
them in parallel.

```json
[
  { "tool": "web_search", "args": { "query": "React framework features 2026", "vertical": "web" } },
  { "tool": "web_search", "args": { "query": "Vue framework features 2026",    "vertical": "web" } },
  { "tool": "web_search", "args": { "query": "Svelte framework features 2026", "vertical": "web" } }
]
```

Each call returns its own `results` array with `citation_index`
reset to 1. The Search strategy / planner is responsible for
merging and re-numbering citations across calls — see the
Citation indices gotcha in `reference/gotchas.md`.

## Combining web_search with web_fetch

**Context**: User asks for "the full text of the Rust 1.85
release post." Search finds it; fetch reads the page.

```json
[
  { "tool": "web_search", "args": { "query": "Rust 1.85 release blog post" } },
  { "tool": "web_fetch",  "args": { "url": "<url-from-search-results>" } }
]
```

Do NOT call `web_fetch` without first knowing the URL — it
needs a fully-qualified URL, not a topic.
