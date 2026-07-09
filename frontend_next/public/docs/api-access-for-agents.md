# API Access for Agents

Stable link: `/docs/api-access-for-agents.md`

This page is the agent-readable entry point for **workspace-scoped** API access.

Context-OS is a **personal knowledge product**: one signed-in user owns their workspaces. There is no team/org administration surface in the product UI. External automation should attach to a **specific workspace** the user already created.

2026-04-26 architecture note:
- Product-side user interaction is owned by Main Agent.
- RAG API is a retrieval service and returns evidence/retrieval bundles to Main Agent.
- This public endpoint document describes the current HTTP access shape; deeper target architecture is documented in [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## Scope
- `workspace` maps to `workspace_id`.
- External agents should prefer the **unified MCP** entry point at `POST /api/v1/mcp`.
- REST/SSE remains available for UI, streaming, and binary uploads.

## Typical personal workflow

1. The human creates a workspace in the product UI.
2. On that workspace's **API Access** page, they create a **workspace API key** (permissions `index` and/or `query`).
3. The agent uses that key plus the workspace's `workspace_id` for upload, indexing, and RAG.

Agents do **not** need a separate “account-level” or “org-level” key for normal personal use.

## Authentication
- **Agents (external automation):** `Authorization: Bearer <workspace_api_key>` on `POST /api/v1/mcp` and scoped REST routes. Permission checks apply to the key (`index`, `query`, etc.).
- **Human UI (interactive sessions):** user JWT (or trusted frontend proxy headers). Signed-in users are not subject to API-key permission strings.
- **Workspace API keys** (scoped to one `workspace_id`): default permissions `index`, `query` when omitted at creation — call `workspace.*` MCP tools only.
- Internal proxy auth (`x-org-id`, `x-user-id`, optional `x-permissions`) is for trusted frontends and tests only. End users and personal agents should use JWT or workspace API keys instead.

Create workspace keys (product path): `POST /api/v1/workspaces/{workspace_id}/api-keys` (signed-in user session only; managed from the workspace API Access UI)

## Unified MCP (preferred)

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/api/v1/mcp` | JSON-RPC: `initialize`, `tools/list`, `tools/call` |
| `GET` | `/api/v1/mcp` | SSE `ready` event with tool catalog summary |

### Workspace tools (require `arguments.workspace_id`)

These are the tools personal agents should use after the human shares a workspace id and key.

| Tool | Permission | Purpose |
| --- | --- | --- |
| `workspace.create_upload` | `index` | Start file upload → returns `upload_url` |
| `workspace.complete_upload` | `index` | Finalize after HTTP PUT |
| `workspace.document_status` | `index` or `query` | Poll until `completed` |
| `workspace.add_url_source` | `index` | Add URL source |
| `workspace.list_sources` | `query` | List sources |
| `workspace.rag_query` | `query` | RAG (codegen/SDK) |
| `workspace.search_query` | `query` | Web search (native tools) |
| `workspace.chat` | `query` | Legacy alias for RAG |

Example `tools/call`:

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "tools/call",
  "params": {
    "name": "workspace.rag_query",
    "arguments": {
      "workspace_id": "11111111-1111-1111-1111-111111111111",
      "query": "Summarize the indexed docs"
    }
  }
}
```

### Upload orchestration (MCP + HTTP)

1. `workspace.create_upload` → `document_id`, `upload_url`
2. `HTTP PUT` bytes to `upload_url`
3. `workspace.complete_upload`
4. Poll `workspace.document_status` until `completed`
5. `workspace.rag_query`

### Wire-protocol note (not part of the personal product UI)

The MCP catalog may still list tools named `org.create_workspace` and `org.list_workspaces`. Those names reflect internal routing, not a user-facing organization feature. **Personal users create workspaces in the UI**, not through agent credentials. Integrators should not depend on account-scoped API keys unless they operate a separate automation platform.

## REST endpoints (advanced / UI)

REST routes below enforce the **same workspace scope and permission rules** as MCP. A workspace API key cannot access another workspace's resources. UI-only routes (share, members, notes, notifications, API key management) require a **user session** — API keys receive `403 api_key_forbidden`.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/sources?workspace_id={workspace_id}` | List sources |
| `POST` | `/api/v1/workspaces/{workspace_id}/documents` | Create file upload |
| `POST` | `/api/v1/documents/{document_id}/complete-upload` | Finish upload |
| `POST` | `/api/v1/chat` | REST/SSE chat |
| `GET` | `/api/v1/agent/operation-guides/{mode}` | Prefetch guide (`rag`, `search`, `index`, `workspace.create`) |

## Deprecated MCP path

- `POST /mcp/workspaces/{workspace_id}` — legacy; injects `workspace_id` from URL. Migrate to `/api/v1/mcp`.

## Progressive disclosure (`agent_operation_guide`)

Every MCP `tools/call` success includes `agent_operation_guide` in `structuredContent`. On failure the HTTP status is still **200**; the body is a JSON-RPC envelope with `error.data.agent_operation_guide` (and `error.data.error` for the business error code). Modes: `rag`, `search`, `index`, `workspace.create`.

REST endpoints keep flat JSON errors with normal HTTP status codes (403/400/etc.).

## Common errors

### MCP `tools/call` (HTTP 200, JSON-RPC `error`)

- `org_key_cannot_call_workspace_tools`: account-scoped automation credential called a workspace tool (personal agents should use a workspace key instead)
- `workspace_key_cannot_call_org_tools`: workspace key called an internal account-provisioning tool (create the workspace in the UI first)
- `notebook_scope_mismatch`: workspace key used for wrong `workspace_id`
- `missing_permission`: API key lacks required permission
- `docscope_required`: RAG with no completed documents
- `rag_runtime_not_configured`: RAG backend not configured in test/dev environments

### REST (HTTP 403/400 flat JSON)

- `api_key_forbidden`: API key attempted a user-only endpoint (key management, profile, preferences, message feedback)
- `notebook_access_required`: signed-in user lacks access to manage resources for this workspace
- `workspace_key_cannot_call_org_tools`, `notebook_scope_mismatch`, `missing_permission`: same semantics as MCP
