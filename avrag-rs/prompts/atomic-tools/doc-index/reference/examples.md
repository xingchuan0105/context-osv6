# Examples

Good call signatures for `doc-index`. Always comes BEFORE
`index_lookup` for the same document.

## Example 1: Plan an `index_lookup` for a specific section

**Context**: User asks "show me the 'Antifragility Connection'
section in the book."

```json
[
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<antifragile-book-uuid>"]
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<antifragile-book-uuid>",
      "chunk_ids": ["<chunks-from-Antifragility-Connection-section>"]
    }
  }
]
```

`doc_index` returns the full TOC with chunk IDs. The planner
finds the "Antifragility Connection" section in the result,
extracts its `chunk_ids`, and passes them to `index_lookup`.

## Example 2: Multi-document section structure

**Context**: User asks "give me a section map of all my uploaded
PDFs."

```json
{
  "tool": "doc_index",
  "version": "1.0",
  "args": {
    "doc_ids": [
      "<pdf-1-uuid>",
      "<pdf-2-uuid>",
      "<pdf-3-uuid>"
    ]
  }
}
```

Pass all `doc_ids` at once. The runtime returns an index per
document. The planner returns the combined structure to the
user, or uses it to plan downstream `index_lookup` calls.

## Example 3: Use index as a lightweight TOC

**Context**: User asks "what's the structure of this PDF?"

```json
{
  "tool": "doc_index",
  "version": "1.0",
  "args": {
    "doc_ids": ["<pdf-uuid>"]
  }
}
```

`doc_index` returns just the section titles + heading levels +
chunk_ids. The planner returns the title list to the user
(without exposing the chunk IDs).

## Example 4: Combine with `doc_metadata`

**Context**: User asks "give me a quick overview of the doc and
its sections."

```json
[
  {
    "tool": "doc_metadata",
    "version": "1.0",
    "args": {
      "doc_ids": ["<doc-uuid>"],
      "fields": ["name", "status", "chunk_count"]
    }
  },
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<doc-uuid>"]
    }
  }
]
```

`doc-metadata` first to verify the doc is ready (status:
`completed`, chunk_count > 0), then `doc-index` for the
section map. Cheap pre-flight.

## Example 5: Skip `doc-index` when chunk IDs are already known

**Context**: Same session as a previous query that already called
`doc-index`. The planner has cached chunk IDs in session state.
The document has NOT been re-ingested.

```json
{
  "tool": "index_lookup",
  "version": "1.0",
  "args": {
    "doc_id": "<doc-uuid>",
    "chunk_ids": ["<cached-ids>"]
  }
}
```

If the planner already has chunk IDs in session state AND the
document has not been re-ingested since, skip `doc-index` and
go directly to `index_lookup`. **If the document was re-ingested,
you MUST call `doc-index` again** — old chunk IDs are invalid.

## Example 6: Stale chunk IDs after re-ingestion

**Context**: A document was re-ingested between sessions. The
planner attempts to use cached chunk IDs from the previous session.

**Step 1** (wrong — old IDs):
```json
{
  "tool": "index_lookup",
  "version": "1.0",
  "args": {
    "doc_id": "<doc-uuid>",
    "chunk_ids": ["<old-stale-id-1>", "<old-stale-id-2>"]
  }
}
```
Result: `[]` (empty — stale IDs no longer exist)

**Step 2** (correct — re-index first):
```json
[
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<doc-uuid>"]
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<doc-uuid>",
      "chunk_ids": ["<new-id-1>", "<new-id-2>"]
    }
  }
]
```

**Rule**: After re-ingestion, always re-call `doc_index` before
`index_lookup`. Cached IDs are invalid.

## Example 7: Error — document not found

**Context**: The `doc_id` does not exist in the workspace.

```json
{
  "tool": "doc_index",
  "version": "1.0",
  "args": {
    "doc_ids": ["non-existent-uuid"]
  }
}
```

Result:
```json
[
  {
    "doc_id": "non-existent-uuid",
    "error": {
      "code": "DOC_NOT_FOUND",
      "message": "Document not found in workspace"
    }
  }
]
```

## Example 8: Error — access denied

**Context**: The caller lacks read permission for the document.

```json
{
  "tool": "doc_index",
  "version": "1.0",
  "args": {
    "doc_ids": ["<restricted-doc-uuid>"]
  }
}
```

Result:
```json
[
  {
    "doc_id": "<restricted-doc-uuid>",
    "error": {
      "code": "ACCESS_DENIED",
      "message": "Caller does not have read access to this document"
    }
  }
]
```

## Example 9: Error — index empty (unparseable document)

**Context**: A scanned PDF without OCR text.

```json
{
  "tool": "doc_index",
  "version": "1.0",
  "args": {
    "doc_ids": ["<scanned-pdf-uuid>"]
  }
}
```

Result:
```json
[
  {
    "doc_id": "<scanned-pdf-uuid>",
    "error": {
      "code": "INDEX_EMPTY",
      "message": "Document has no extractable sections"
    }
  }
]
```
