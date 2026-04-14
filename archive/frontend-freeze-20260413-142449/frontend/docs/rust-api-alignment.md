# Frontend Rust API Alignment Notes

## Scope

This document defines the frontend alignment target for `M1 + M2`.

- `M1`: Rust workspace boots, HTTP/SSE foundation is available, health/docs routes exist.
- `M2`: notebook management, document upload/status, chat streaming, citations, and source-aware workspace UI can run against Rust APIs.

The PRD uses `notebook` as the canonical product and API term. The current UI still uses `workspace` and `KnowledgeBase` in many local names. For this batch:

- Keep existing page structure and most component names unchanged.
- Treat `workspace` as a UI label only.
- Treat `notebook` as the backend contract term.
- Avoid broad renames until Rust API integration is stable.

## Current Frontend Contract Baseline

The existing frontend already targets most of the required API surface:

- `GET/POST/PUT/DELETE /api/v1/notebooks`
- `POST /api/v1/notebooks/{id}/documents`
- `GET /api/v1/documents/{doc_id}/status`
- `DELETE /api/v1/documents/{doc_id}`
- `POST /api/v1/documents/{doc_id}/reindex`
- `POST /api/v1/chat`
- `GET /api/v1/chat/sessions`
- `GET /api/v1/chat/sessions/{sessionId}`
- `GET /api/v1/chat/sessions/{sessionId}/messages`
- `POST /api/v1/chat/citations/lookup`
- `GET/POST/DELETE /api/v1/notebooks/{id}/api-keys`

Primary integration points:

- `src/lib/api/client.ts`
- `src/components/chat/chat-panel.tsx`
- `src/components/chat/chat-bubble.tsx`
- `src/app/dashboard/page.tsx`
- `src/app/dashboard/[id]/page.tsx`
- `src/components/dashboard/api-access-modal.tsx`

## Naming Rule For M1 + M2

Use these conventions when touching frontend code in this batch:

- API path params and request/response fields: use `notebook`.
- Existing UI state/store/component names: may continue using `workspace` for now.
- Existing model type `KnowledgeBase`: keep as a compatibility alias for notebook-shaped data.
- Existing `kb_id` fields in UI types: keep temporarily, but treat them as `notebook_id`.

This gives us a stable bridge while avoiding a wide UI rename during backend migration.

## Required Rust Response Compatibility

### Notebook responses

Frontend currently expects either:

- `response.data.notebooks`
- `response.data.notebook`

Each notebook item must support:

- `id`
- `title` or `name`
- `description`
- `icon`
- `created_at`

## Chat response envelope

Rust must preserve the PRD envelope shape used by the frontend:

```json
{
  "answer": "string",
  "session_id": "string",
  "agent_type": "rag|general|search",
  "sources": [],
  "citations": [],
  "trace": {},
  "degrade_trace": [],
  "planner_output": {},
  "mode_debug": {}
}
```

Frontend uses:

- `answer`
- `session_id`
- `agent_type`
- `sources`
- `citations`
- `degrade_trace`
- `planner_output`
- `mode_debug.rag.*`

### SSE events

The chat runtime already parses SSE-style events and must continue receiving:

- `start`
- `token`
- `citations`
- `trace`
- `done`
- `error`
- `planner_complete`
- `rag_trace`
- `rag_sources`

Search mode extensions should remain additive:

- `search_start`
- `search_results`

## M1 + M2 Frontend Integration Checklist

### Track A: foundation verification

- Confirm Rust dev server proxy targets in `next.config.ts`.
- Verify `GET /health`, `GET /ready`, and docs/OpenAPI routes for local dev.
- Add a frontend env note when Rust API base URL differs from the Go stack.

### Track B: notebook and document flow

- Validate notebook list/detail/create/update/delete against Rust JSON shapes.
- Validate document upload create step against Rust upload contract.
- Validate document status polling values:
  - `pending`
  - `enqueueing`
  - `queued`
  - `processing`
  - `completed`
  - `failed`
- Validate parsed preview/content payload compatibility for source viewer.

### Track C: chat flow

- Validate `POST /api/v1/chat` non-stream response.
- Validate streaming response framing and event order.
- Validate `session_id` lifecycle for first message vs resumed session.
- Validate citation lookup payloads and fallback behavior.
- Validate `degrade_trace` rendering and rag debug panel behavior.

## Known Blockers To Resolve In Rust

1. Upload contract is still implementation-sensitive.
   The current frontend posts to `/api/v1/notebooks/{kbId}/documents` and expects a create/upload flow compatible with `documentsApi.upload`.

2. Source viewer payloads are not yet frozen.
   `DocumentViewer` depends on `/content` and `/parsed-preview`; Rust should preserve one of these payload shapes early.

3. Search result navigation still uses `workspace_id`.
   Search endpoints may need a temporary compatibility field or a frontend follow-up patch once Rust search lands.

4. Chat agent naming still uses `knowledge_base`.
   Frontend maps this to `rag`, but Rust agent catalogs should either return `knowledge_base` for compatibility or we should schedule a small follow-up normalization patch.

## Recommended Integration Order

1. Health/readiness/docs/OpenAPI
2. Notebook CRUD
3. Document upload + status polling
4. Document viewer payloads
5. Chat non-stream
6. Chat SSE + citations + degrade trace
7. API keys panel

## Non-goals In This Batch

- No large-scale rename from `workspace` to `notebook` across the whole UI.
- No route restructuring.
- No visual redesign.
- No front-end-only search contract cleanup before Rust search mode is ready.
