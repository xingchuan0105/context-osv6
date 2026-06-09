# Output Schema

## Success response

```json
{
  "url": "https://example.com/article",
  "title": "Example Article Title",
  "content": "The extracted main text of the page...",
  "truncated": false,
  "length": 4521
}
```

### Field details

| Field | Type | Description |
|-------|------|-------------|
| `url` | string | Echoes back the requested URL for citation and verification. |
| `title` | string | The `<title>` tag content, if extractable. May be empty if the page lacks a title tag. |
| `content` | string | The primary readable text of the page, with HTML tags, scripts, styles, and common boilerplate removed. Whitespace is normalized. |
| `truncated` | boolean | `true` if `content` exceeded `max_length` and was cut. |
| `length` | integer | Full character count before truncation. Helps the agent decide whether to request a higher `max_length`. |

## Error handling contract

All failure modes return a structured error object. The caller MUST NOT assume success; check `status` before reading `content`.

```json
{
  "status": "error",
  "error": {
    "code": "INVALID_URL | NETWORK_ERROR | TIMEOUT | BLOCKED | EXTRACTION_FAILED | HTTP_ERROR",
    "message": "Human-readable description of what went wrong.",
    "url": "https://the-url-that-failed.com"
  }
}
```

### Error codes

| Code | When it happens | Caller action |
|------|-----------------|---------------|
| `INVALID_URL` | Missing `url`, empty string, malformed URL, or unsupported scheme (`ftp://`, `file://`). | Ask the user for a valid HTTP/HTTPS URL. |
| `BLOCKED` | Private address (`localhost`, `127.0.0.1`, RFC1918) or URL on the denylist. | Inform the user the URL is not accessible. |
| `NETWORK_ERROR` | DNS failure, connection refused, SSL error. | Retry once; if persistent, inform the user the site is unreachable. |
| `TIMEOUT` | Page took longer than the configured timeout to respond. | Retry once; if persistent, inform the user the site is slow or down. |
| `HTTP_ERROR` | HTTP 4xx/5xx response from the server. | Surface the status code to the user (e.g., "404 Not Found"). |
| `EXTRACTION_FAILED` | HTML could not be parsed or the page body is empty after boilerplate removal (common on JS-only SPAs). | Inform the user the page could not be read, possibly due to JavaScript rendering. |

### Empty-content edge case

When the page loads successfully but yields no extractable text (e.g., a JS-only SPA), the response is **not** an error. It returns success with `content: ""`, `length: 0`, and `title` populated if available. The caller should treat this as a signal that the page is JS-dependent.
