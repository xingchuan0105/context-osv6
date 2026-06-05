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

If the planner already has chunk IDs in session state, skip
`doc-index` and go directly to `index_lookup`. (The cache must
be invalidated on re-ingestion.)
