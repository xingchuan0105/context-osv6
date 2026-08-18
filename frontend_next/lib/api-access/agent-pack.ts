/**
 * One-shot Agent Pack builder — paste into Cursor / Claude / external agents.
 * Field names stay English for machine parsing; surrounding prose is English by design.
 */

export type AgentPackInput = {
  workspaceId: string;
  apiBase: string;
  /** One-time plaintext workspace API key; omit when not available. */
  apiKey?: string | null;
  permissions?: string[];
  /** Absolute origin for docs links (browser origin or api base). */
  docsOrigin: string;
  runtime?: "cloud" | "desktop-local";
};

export const AGENT_DOCS_PATH = "/help/api-access/agents";
export const HUMAN_DOCS_PATH = "/help/api-access";

export function absoluteDocUrl(origin: string, docPath: string): string {
  const base = origin.replace(/\/$/, "");
  const path = docPath.startsWith("/") ? docPath : `/${docPath}`;
  return `${base}${path}`;
}

export function buildAgentPack(input: AgentPackInput): string {
  const apiBase = input.apiBase.replace(/\/$/, "");
  const mcpHttp = `${apiBase}/api/v1/mcp`;
  const docsHuman = absoluteDocUrl(input.docsOrigin, HUMAN_DOCS_PATH);
  const docsAgent = absoluteDocUrl(input.docsOrigin, AGENT_DOCS_PATH);
  const hasKey = Boolean(input.apiKey?.trim());
  const apiKey = hasKey ? input.apiKey!.trim() : "";
  const perms =
    input.permissions && input.permissions.length > 0
      ? input.permissions.join(",")
      : "index,query";
  const runtime = input.runtime ?? "cloud";

  const mcpJson = {
    mcpServers: {
      "context-os": {
        command: "context-os-mcp",
        env: {
          CONTEXT_OS_API_BASE: apiBase,
          CONTEXT_OS_API_KEY: hasKey ? apiKey : "<CREATE_KEY_FIRST>",
          CONTEXT_OS_WORKSPACE_ID: input.workspaceId,
        },
      },
    },
  };

  const lines = [
    "# Context OS — Workspace Agent Pack",
    "# Paste this whole block into your agent / MCP client. Do not commit secrets to git.",
    "",
    "## Connection",
    `- product: Context OS`,
    `- workspace_id: ${input.workspaceId}`,
    `- api_base: ${apiBase}`,
    `- mcp_http: ${mcpHttp}`,
    hasKey ? `- api_key: ${apiKey}` : `- api_key: <CREATE_KEY_FIRST>`,
    hasKey
      ? `- auth: Authorization: Bearer ${apiKey}`
      : `- auth: Authorization: Bearer <CREATE_KEY_FIRST>`,
    `- permissions: ${perms}`,
    `- runtime: ${runtime}`,
    hasKey ? `- status: ready` : `- status: missing_key`,
    "",
    "## MCP (Cursor / Claude Code) — stdio wrapper",
    "```json",
    JSON.stringify(mcpJson, null, 2),
    "```",
    "",
    "## Notes",
    "- Pass workspace_id on every tools/call arguments (same as above).",
    "- Workspace keys: index/query only. Share admin needs a user session (not this key).",
    runtime === "desktop-local"
      ? "- Desktop: start the local client stack before connecting (default 127.0.0.1:18080)."
      : "- Cloud: mcp_http is for MCP clients, not a browser page.",
    "",
    "## Docs",
    `- human: ${docsHuman}`,
    `- agent: ${docsAgent}`,
    "",
    "## Probe",
    hasKey
      ? `curl -sS -X POST "${mcpHttp}" \\
  -H "Authorization: Bearer ${apiKey}" \\
  -H "Content-Type: application/json" \\
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'`
      : `# Create a workspace API key in the product UI, then re-copy this pack.`,
    "",
  ];

  return lines.join("\n");
}
