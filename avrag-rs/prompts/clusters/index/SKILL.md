---
name: index
description: "Load when ingesting documents into a workspace via API or MCP: file upload, URL source, completion, and status polling."
disclose_at: runtime
atomic: true
---

## Document ingestion (workspace-scoped)

### File upload flow

1. Call `workspace.create_upload` with `notebook_id`, `filename`, `mime_type`, `file_size`.
2. HTTP `PUT` the file bytes to the returned `upload_url` (do not embed large files in MCP JSON).
3. Call `workspace.complete_upload` with `document_id`.
4. Poll `workspace.document_status` until status is `completed` before RAG queries.

### URL source flow

1. Call `workspace.add_url_source` with `notebook_id` and `url`.
2. Poll `workspace.document_status` until `completed`.

### Rules

- All ingestion tools require `notebook_id` matching the workspace API key scope.
- Use `workspace.list_sources` to inspect indexed sources before querying.
- After ingestion completes, use `workspace.rag_query` with `agent_type=rag` (codegen/SDK), not search tools.

## Forbidden

- Do not skip `complete_upload` after PUT.
- Do not call RAG before document status is `completed`.
