# Args Schema

The full JSON Schema for `web_fetch` args, as enforced by the runtime at the call boundary.

```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "The fully-qualified URL to fetch. Must start with http:// or https://."
    },
    "max_length": {
      "type": "integer",
      "default": 8000,
      "description": "Maximum characters to return. Longer content is truncated."
    }
  },
  "required": ["url"]
}
```

## Field details

### `url` (required, string)

The URL to fetch. Must be a publicly accessible HTTP or HTTPS URL.

**Good**:
- "https://blog.rust-lang.org/2025/01/01/Rust-1.85.0.html"
- "https://en.wikipedia.org/wiki/Rust_(programming_language)"

**Bad**:
- "" — empty URL (runtime error)
- "ftp://files.example.com/data.txt" — unsupported scheme
- "http://localhost:8080/internal" — private address blocked
- "example.com" — missing scheme

### `max_length` (optional, default 8000)

Cap the returned content length to avoid overflowing the context window. The runtime strips HTML boilerplate before applying this limit.

- Use a lower value (e.g. 4000) when only a brief summary is needed.
- Use a higher value (e.g. 16000) when detailed reading is required.
