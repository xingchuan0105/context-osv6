# Examples

Good call signatures for `doc-metadata`.

## Example 1: Quick "is it ready" check

**Context**: Before a long retrieval, the planner wants to
confirm the document is processed.

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"],
    "fields": ["status", "chunk_count"]
  }
}
```

Single document, minimal fields. If `status` is `completed`
and `chunk_count > 0`, proceed. Otherwise the planner should
refuse or fall back.

## Example 2: "What kind of file is this?"

**Context**: User asks "is the manual a PDF or DOCX?"

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"],
    "fields": ["name", "mime_type", "file_size"]
  }
}
```

Just the file-type fields. The planner returns `name` and
`mime_type` directly to the user. `file_size` is in bytes —
format to human-readable (e.g., "2.3 MB") before displaying.

## Example 3: Get the TOC for a long doc

**Context**: User asks "what sections does the engineering
handbook have?" — want the section list without chunk IDs.

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<handbook-uuid>"],
    "fields": ["toc"]
  }
}
```

`fields: ['toc']` returns the section list (title, heading_level,
page, rank) WITHOUT chunk IDs. Cheaper than `doc-index` if the
planner doesn't plan a follow-up `index_lookup`.

## Example 4: Full metadata dump (admin / debug)

**Context**: User asks "show me everything about this file."

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"]
  }
}
```

Omit `fields` for the complete metadata object. Use this for
admin / debug / "what is this file" UIs.

## Example 5: Multi-doc status check

**Context**: User asks "are my uploads done processing?"

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": [
      "<upload-1-uuid>",
      "<upload-2-uuid>",
      "<upload-3-uuid>"
    ],
    "fields": ["name", "status", "chunk_count"]
  }
}
```

One call, multiple docs, minimal fields. Cheap way to show a
"processing status" dashboard.

## Example 6: `fields: []` is equivalent to omitting

**Context**: Explicitly passing an empty fields array.

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"],
    "fields": []
  }
}
```

Result is identical to omitting `fields` entirely — returns the
complete metadata object. Both forms are valid.

## Example 7: Failed document blocks retrieval

**Context**: User asks a question about a document that failed ingestion.

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"],
    "fields": ["name", "status"]
  }
}
```

Result:
```json
[
  {
    "doc_id": "<doc-uuid>",
    "name": "corrupted.pdf",
    "status": "failed"
  }
]
```

**Action**: The planner MUST refuse to proceed with retrieval.
Inform the user: "The document 'corrupted.pdf' failed processing
and cannot be searched. Please re-upload it."

## Example 8: Pending document — wait, don't retrieve

**Context**: User asks a question about a recently uploaded document.

```json
{
  "tool": "doc_metadata",
  "version": "1.0",
  "args": {
    "doc_ids": ["<doc-uuid>"],
    "fields": ["name", "status", "chunk_count"]
  }
}
```

Result:
```json
[
  {
    "doc_id": "<doc-uuid>",
    "name": "new_upload.pdf",
    "status": "processing",
    "chunk_count": 0
  }
]
```

**Action**: The planner MUST NOT call retrieval tools. Inform the
user: "The document is still being processed. Please wait a moment
and try again."
