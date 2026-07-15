# Unified synthesis contract (`internal_answer_unified_v1`)

**Date:** 2026-07-15  
**Status:** Implemented (backend primary path; frontend marker render)

## Decision

After dual RAG+Search handtest failures (raw JSON envelopes, mangled keys, mixed cite styles):

1. **One synthesis schema** for rag-only, search-only, and dual: `internal_answer_unified_v1`
2. **Body markers:** `[[cite:CHUNK_ID]]` for documents; **`[[web:n]]`** for web (not bare `[[n]]` as the preferred form; legacy `[[n]]` still rewritten server-side)
3. **One parser path** with key/value normalization for model mangling (`schemaversion` → `schema_version`, etc.)
4. User-facing output is always **`answer_text` prose**, never the JSON envelope

## Wire shape

```json
{
  "schema_version": "internal_answer_unified_v1",
  "answer_text": "…[[cite:abc]]…[[web:1]]…",
  "citations": [
    { "kind": "doc", "id": "abc" },
    { "kind": "web", "id": "1", "url": null, "title": null }
  ],
  "coverage": "full",
  "refusal_reason": null
}
```

## Mode assembly

| Capabilities | Contract |
|--------------|----------|
| `[]` pure chat | `prose_only` |
| rag and/or search | `internal_answer_unified_v1` |

Legacy `internal_answer_v1` / `internal_search_answer_v1` / hybrid still **accepted as input** and upgraded to unified where possible.

## Frontend

`citation-renderer` recognizes `[[web:n]]` alongside `[[cite:]]` and legacy `[[n]]`.
