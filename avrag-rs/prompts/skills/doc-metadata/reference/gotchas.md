# Gotchas

## Status gate — the hard rule

Only `status: "completed"` documents may proceed to expensive retrieval.

| `status` | Meaning | Retrieval allowed? |
|----------|---------|-------------------|
| `completed` | Indexed and ready | ✅ Yes |
| `pending` | Queued for processing | ❌ No — inform user to wait |
| `processing` | Currently being indexed | ❌ No — inform user to wait |
| `failed` | Ingestion failed | ❌ No — inform user to re-upload |

**Never** call `dense-retrieval`, `lexical-retrieval`, `doc-index`, or `index_lookup` on a document that is not `completed`.

## Fields filter semantics

- Omit `fields` → returns the complete metadata object.
- `fields: []` → equivalent to omitting `fields` (returns all fields).
- `fields: ["name", "status"]` → returns only the requested fields.
- Unknown field names are silently ignored.

## Machine-friendly fields

- `file_size` is in bytes. Format to human-readable units (KB, MB) before presenting to users.
- `chunk_count` is approximate during re-ingestion. A document just re-uploaded may show the old count for a few seconds. Do not rely on it for exact arithmetic during active re-ingest.

## Content boundary

This tool reads metadata only. It does NOT return document content (text, page text, or chunk bodies). For content, use `dense-retrieval`, `lexical-retrieval`, `index_lookup`, or `doc-summary`.

## Empty input

Empty `doc_ids` array returns empty metadata.

## TOC vs doc-index

- `doc-metadata` with `fields: ["toc"]` returns section titles, heading levels, and page numbers.
- `doc-index` returns the same PLUS chunk IDs for `index_lookup`.
- **Use `doc-metadata` for TOC display only. Use `doc-index` when you need chunk IDs for precise reading.**
