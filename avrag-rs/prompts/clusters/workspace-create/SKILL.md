---
name: workspace-create
description: "Load when an agent needs a workspace id for MCP automation. Personal users create workspaces in the product UI."
disclose_at: runtime
atomic: true
---

## Personal product workflow

Context-OS is a personal product: one user owns their workspaces. Agents should **not** assume an account or account-level API key.

### Get a workspace id

1. Ask the human to create a workspace in the product UI (or use one they already opened).
2. Copy the workspace id (`workspace_id`) from the URL or API Access page.
3. Ask them to create a **workspace-scoped API key** on that workspace's API Access page with `index` and `query`.
4. Use that key for ingestion and RAG via `workspace.*` MCP tools.

### Do not use for personal agents

- Do not mint or require account-scoped / org-scoped API keys for normal personal automation.
- MCP tools named `account.create_workspace` / `account.list_workspaces` are internal wire names, not part of the personal product surface.

## Forbidden

- Workspace-scoped API keys cannot call `org.*` tools.
- Do not pass a workspace id the key was not scoped to.
