# Examples

Good call signatures for `index-lookup`. Always preceded by
`doc-index` (or a cached chunk-ID list from session state).

## Example 1: Fetch a specific section's chunks

**Context**: After `doc-index` returned the section structure, the
planner found the "Definition" section has chunk IDs
`["uuid-1", "uuid-2", "uuid-3"]`.

```json
{
  "tool": "index_lookup",
  "version": "1.0",
  "args": {
    "doc_id": "<doc-uuid>",
    "chunk_ids": ["uuid-1", "uuid-2", "uuid-3"]
  }
}
```

Direct read. Returns exactly the 3 chunks in the same order.

## Example 2: User names a specific section

**Context**: User asks "show me the 'Lindy Effect' section in
lindy.txt."

```json
[
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<lindy-txt-uuid>"]
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<lindy-txt-uuid>",
      "chunk_ids": ["<chunks-from-Lindy-Effect-section>"]
    }
  }
]
```

`doc-index` returns the full TOC; planner finds the "Lindy
Effect" section's chunk_ids; `index_lookup` fetches them.

## Example 3: Re-read a chunk from session cache

**Context**: Same session, the planner earlier called
`doc-index` and cached the chunk IDs. Now the user asks a
follow-up about the same section.

```json
{
  "tool": "index_lookup",
  "version": "1.0",
  "args": {
    "doc_id": "<doc-uuid>",
    "chunk_ids": ["<cached-ids-from-doc-index>"]
  }
}
```

Skip `doc-index` (already in session cache). If the doc was
re-ingested since the cache was built, the IDs may be stale —
the planner should invalidate the cache and re-call `doc-index`.

## Example 4: Surgical read of a single chunk

**Context**: User asks "what does chunk 42 say?"

```json
{
  "tool": "index_lookup",
  "version": "1.0",
  "args": {
    "doc_id": "<doc-uuid>",
    "chunk_ids": ["<chunk-42-uuid>"]
  }
}
```

`minItems: 1` so single-chunk reads work. The runtime returns
a 1-element array.

## Example 5: Combine with `graph-retrieval`

**Context**: User asks "show me the chunks that support the
relationship between X and Y."

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "graph_hints": [
        { "subject": "X", "predicate": "related_to", "object": "Y" }
      ],
      "supporting_chunk_limit": 5
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<doc-uuid>",
      "chunk_ids": ["<chunk-ids-from-graph-supporting-chunks>"]
    }
  }
]
```

`graph-retrieval` returns relations + supporting chunks; the
planner takes the supporting chunk IDs and passes them to
`index_lookup` for the actual text.
