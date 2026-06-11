# Examples

Good call signatures for `doc-summary`.

## Example 1: Pre-retrieval scoping (level: "doc")

**Context**: User asks "what does the manual say about the
deployment process?" — planner doesn't know if "the manual"
is one of many docs.

```json
[
  {
    "tool": "doc_metadata",
    "version": "1.0",
    "args": {
      "doc_ids": ["<candidates>"],
      "fields": ["name", "status"]
    }
  },
  {
    "tool": "doc_summary",
    "version": "1.0",
    "args": {
      "doc_ids": ["<candidates>"],
      "level": "doc"
    }
  },
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["deployment process steps procedure"],
      "top_k": 10
    }
  }
]
```

`doc-metadata` filters to docs that are `status: completed`,
then `doc-summary` gives a narrative map of each, then
`dense-retrieval` for the actual content. The three-step pre-flight
prevents the dense call from wasting budget on incomplete docs.

## Example 2: Section-level planning (level: "section")

**Context**: User asks "the 'Authentication' section in the API
guide" — planner needs the section's chunk IDs before
`index_lookup`.

```json
[
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<api-guide-uuid>"]
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<api-guide-uuid>",
      "chunk_ids": ["<chunks-from-Authentication-section>"]
    }
  }
]
```

Note: `doc-index` returns chunk IDs directly (level: "section" via
`doc_summary` would only return `section_title` values, not chunk IDs).
For chunk-precise reads, use `doc-index` not `doc-summary`.

## Example 3: Direct overview (user asks for a summary)

**Context**: User asks "give me a one-paragraph overview of the
fraud detection system."

```json
{
  "tool": "doc_summary",
  "version": "1.0",
  "args": {
    "doc_ids": ["<fraud-detection-doc-uuid>"],
    "level": "doc"
  }
}
```

`level: "doc"` returns the narrative summary. The planner
returns this directly as the answer (or as input to a chat-mode
LLM for further synthesis).

## Example 4: Multi-doc comparison

**Context**: User asks "compare the auth flows in our iOS and
Android docs".

```json
[
  {
    "tool": "doc_summary",
    "version": "1.0",
    "args": {
      "doc_ids": ["<ios-auth-doc>", "<android-auth-doc>"],
      "level": "doc"
    }
  }
]
```

Fetch summaries for both docs. The planner returns both to the
user or hands them to `dense-retrieval` for deeper comparison.

## Example 5: Section-level TOC (for a long doc)

**Context**: User asks "what sections does the engineering
handbook have?"

```json
{
  "tool": "doc_summary",
  "version": "1.0",
  "args": {
    "doc_ids": ["<handbook-uuid>"],
    "level": "section"
  }
}
```

`level: "section"` returns TOC entries (`section_title`,
`heading_level`, `page`), one per section. The planner returns
this as a list to the user, OR uses it to plan a follow-up
`doc-index` → `index_lookup` for a specific section.
