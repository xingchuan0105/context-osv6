# Examples

## Positive Example 1 — Summarize a blog post

**User**: "Can you summarize https://blog.rust-lang.org/2025/01/09/Rust-1.85.0.html ?"

**Planner decision**:
```json
{
  "action": "answer",
  "calls": [
    {
      "tool": "web_fetch",
      "args": {
        "url": "https://blog.rust-lang.org/2025/01/09/Rust-1.85.0.html",
        "max_length": 8000
      }
    }
  ]
}
```

**Tool result** (excerpt):
```json
{
  "url": "https://blog.rust-lang.org/2025/01/09/Rust-1.85.0.html",
  "title": "Announcing Rust 1.85.0",
  "content": "The Rust team is happy to announce a new version of Rust, 1.85.0...",
  "truncated": false,
  "length": 3240
}
```

**Agent answer**: "Rust 1.85.0 was released on January 9, 2025. Key highlights include ..."

---

## Positive Example 2 — Deep read of a long technical doc

**User**: "Read the full Kubernetes API conventions doc at https://github.com/kubernetes/community/blob/master/contributors/devel/sig-architecture/api-conventions.md and tell me the rules around field naming."

**Planner decision**:
```json
{
  "action": "answer",
  "calls": [
    {
      "tool": "web_fetch",
      "args": {
        "url": "https://github.com/kubernetes/community/blob/master/contributors/devel/sig-architecture/api-conventions.md",
        "max_length": 16000
      }
    }
  ]
}
```

---

## Positive Example 3 — Search result deep-dive

**User**: "Find recent news about OpenAI and read the most relevant article."

**Step 1**: Planner calls `web_search` with `{"query": "OpenAI recent news 2025", "vertical": "news"}`.

**Step 2**: Search returns 5 results. Planner selects result #1 (most relevant).

**Step 3**: Planner calls `web_fetch` with `{"url": "https://...", "max_length": 8000}`.

**Step 4**: Agent synthesizes answer from fetched content.

---

## Negative Example — No URL provided

**User**: "What are the latest features in Rust?"

**Planner decision** (correct):
```json
{
  "action": "answer",
  "calls": [
    {
      "tool": "web_search",
      "args": {
        "query": "Rust latest features 2025"
      }
    }
  ]
}
```

**Why `web_fetch` is wrong here**: The user did not provide a URL or name a specific page. `web_fetch` requires a target URL. Use `web_search` to discover relevant pages first, then optionally `web_fetch` one result.
