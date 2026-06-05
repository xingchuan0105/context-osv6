# Args Schema

The full JSON Schema for `web_search` args, as enforced by the runtime at the call boundary.

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Standalone search-engine-ready query."
    },
    "vertical": {
      "type": "string",
      "enum": ["web", "news"],
      "default": "web",
      "description": "Search vertical: 'web' for general, 'news' for time-sensitive."
    }
  },
  "required": ["query"]
}
```

## Field details

### `query` (required, string)

A standalone, keyword-rich search query. Must be optimized for a search engine, not copied verbatim from conversational user text.

**Good**:
- "Rust 1.85 release date features"
- "OpenAI GPT-5 announcement 2025"
- "Beijing air quality today"

**Bad**:
- "" — empty query (runtime error)
- "Can you tell me what the latest version of Rust is?" — conversational, not search-optimized
- "rust" — too broad, low signal

### `vertical` (optional, default "web")

- `"web"` (default): General web search. Best for facts, documentation, how-tos.
- `"news"`: News-specific search. Best for breaking events, recent announcements, time-sensitive topics.

**When to use `"news"`**:
- The query is about an event that happened in the last few days.
- The user explicitly asks for "latest news" or "recent developments".
- A general web search returns stale results.

## Output schema

```json
{
  "type": "object",
  "properties": {
    "query_type": { "type": "string", "description": "The type of query executed." },
    "sub_queries": { "type": "array", "items": { "type": "string" }, "description": "Sub-queries if query was decomposed." },
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "title": { "type": "string" },
          "url": { "type": "string" },
          "snippet": { "type": "string" },
          "citation_index": { "type": "integer" }
        }
      }
    },
    "synthesized_answer": { "type": "string", "description": "Provider-generated synthesis (if available)." },
    "llm_usage": { "type": "object", "description": "Token usage metadata (if available)." }
  }
}
```

## Citation indices

Each result has a `citation_index` (starting from 1). Use these indices when citing sources in your final answer (e.g., "According to [1], ...").
