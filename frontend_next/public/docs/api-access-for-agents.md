# API Access for Agents

Stable link: `/docs/api-access-for-agents.md`

This page is the agent-readable entry point for workspace-scoped API access.

2026-04-26 architecture note:
- Product-side user interaction is owned by Main Agent.
- RAG API is a retrieval service and returns evidence/retrieval bundles to Main Agent.
- This public endpoint document describes the current HTTP access shape; deeper target architecture is documented in [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## Scope
- `workspace` maps to `notebook_id`.
- This doc only covers workspace-scoped `sources` management and RAG query.
- `agent_type: "general"` and `agent_type: "search"` are out of scope here.

## Key endpoints
| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/sources?notebook_id={notebook_id}` | List sources for the workspace |
| `POST` | `/api/v1/notebooks/{notebook_id}/sources/url` | Add a URL source |
| `POST` | `/api/v1/notebooks/{notebook_id}/documents` | Create a file upload source |
| `GET` | `/api/v1/documents/{document_id}/status` | Poll ingest/indexing status |
| `POST` | `/api/v1/documents/{document_id}/complete-upload` | Finish a file upload |
| `POST` | `/api/v1/chat` | RAG query only; send `notebook_id` and `agent_type: "rag"` |

## RAG query rules
- Always pass the current workspace as `notebook_id`.
- Use `agent_type: "rag"`.
- Use `stream: true` when you want SSE token streaming.
- Do not use this doc for general chat or global search agents.

## Common errors
- `400 invalid_request`: bad payload, missing required field, or unsupported combination.
- `401 unauthorized`: missing or invalid auth.
- `403 forbidden`: auth is valid, but the workspace/action is not allowed.
- `404 notebook_not_found`: workspace id is invalid or not accessible.
- `404 document_not_found`: document or source id is invalid or not accessible.
- `409 conflict`: state collision, duplicate write, or invalid transition.
- `429 rate_limited`: request throttled.
- `500 internal_error`: backend or retrieval failure.
