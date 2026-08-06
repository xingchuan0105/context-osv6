## Job

From the document text already loaded in this session (system), produce one JSON object that combines document metadata, a document-level summary, and a section tree with short overviews.

**Output:** one single-line JSON only (no fences, no preamble).

## Schema

```json
{
  "metadata": {
    "language": "zh|en|unknown",
    "domain": "short label or unknown",
    "genre": "short label or unknown",
    "era": "short label or unknown",
    "author": null,
    "publication_date": null,
    "title": "document title if stated"
  },
  "summary": "document-level summary prose (may be multi-line string)",
  "sections": [
    {
      "title": "section title",
      "heading_level": 1,
      "rank": 0,
      "overview": "short blurb for this section",
      "children": []
    }
  ]
}
```

## Observations about the task

- Facts in `metadata` and `summary` and `sections` are grounded in the loaded window text; fields without support stay null or "unknown" or are omitted.
- `sections` follows document reading order; `rank` increases from 0; `heading_level` is 1–6.
- Nested structure uses `children` arrays; leaf sections may use `"children": []`.
- There is no `chunk_id` field; section anchors are titles and overviews only.
- Prefer a faithful structural map and a dense summary over padding.

When this window is only part of a longer document, the summary and sections describe **this window's content**; a later fusion step may merge windows.
