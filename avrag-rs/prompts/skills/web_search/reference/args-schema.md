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
- "Rust latest release date features"
- "OpenAI GPT-5 announcement"
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

**Time bounds for `"news"`**:
Provider-dependent (Brave News: last 30 days, weighted toward
the most recent). For events older than ~30 days, prefer
`"web"` and add an explicit year/date token to the query.

## Output schema

```json
{
  "type": "object",
  "properties": {
    "sub_queries": { "type": "array", "items": { "type": "string" }, "description": "Sub-queries actually issued to the provider." },
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
    "synthesized_answer": { "type": ["string", "null"], "description": "Provider-generated synthesis (if available), otherwise null." }
  }
}
```

### `sub_queries` (array of strings)

The sub-queries that were actually issued to the provider.

- In **Search mode**, the Search strategy may decompose a
  user query into multiple sub-queries; this array contains
  each one.
- In **Chat / RAG mode**, this array is `[query]` (one entry,
  the original query).
- Use this to surface to the user what was actually searched,
  especially in Search mode where the decomposition is not
  visible in the original user message.

### `results` ordering

Results are returned in the order the provider ranked them,
typically by relevance (descending). The `citation_index`
field follows this order. There is no per-result `score`
exposed — if you need a confidence signal, treat top-3
results as the high-confidence band and beyond top-10 as
speculative.

**Sponsored / promoted results** (if any) are usually
interleaved by the provider; the tool does NOT strip them.
Check `url` host before citing if sponsorship matters to
the user.

## Citation indices

Each result has a `citation_index` (starting from 1). Cite as
`[1]`, `[2]`, etc. inline, with a numbered reference list at
the end of the answer containing `title — url`.

## Error response

```json
{
  "status": "error",
  "error": {
    "code": "MISSING_QUERY | NO_PROVIDER_CONFIGURED | PROVIDER_UNREACHABLE | API_KEY_INVALID | RATE_LIMITED | INVALID_VERTICAL | NETWORK_TIMEOUT",
    "message": "Human-readable description."
  }
}
```

### Error codes

| Code | When | Caller action |
|------|------|---------------|
| `MISSING_QUERY` | `query` is empty or absent. | Fix caller; do not retry. |
| `NO_PROVIDER_CONFIGURED` | No search backend is configured on the server. | Server-side issue; do not retry. Inform the user. |
| `PROVIDER_UNREACHABLE` | The configured provider (Brave etc.) is unreachable or returned 5xx. | Retry once after a short delay. If persistent, inform the user. |
| `API_KEY_INVALID` | The provider rejected the API key (401/403). | Server-side issue; do not retry. |
| `RATE_LIMITED` | Provider returned 429. | Back off; do not flood. The runtime retries once with backoff internally. |
| `INVALID_VERTICAL` | `vertical` is not `"web"` or `"news"`. | Fix caller. |
| `NETWORK_TIMEOUT` | Network or upstream timeout. | Retry once; if persistent, fall back to a different tool. |
