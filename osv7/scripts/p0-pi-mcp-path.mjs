#!/usr/bin/env node
/**
 * P0: prove the same stdio MCP path pi-mcp-adapter uses can call hello-retrieval.
 * Uses @modelcontextprotocol/sdk shipped inside the installed pi-mcp-adapter package.
 */
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { pathToFileURL } from "node:url";

const adapterRoot =
  process.env.PI_MCP_ADAPTER_ROOT ||
  "/home/chuan/.nvm/versions/node/v24.13.0/lib/node_modules/pi-mcp-adapter";
const sdkClient = join(
  adapterRoot,
  "node_modules/@modelcontextprotocol/sdk/dist/esm/client/index.js",
);
const sdkStdio = join(
  adapterRoot,
  "node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js",
);

const { Client } = await import(pathToFileURL(sdkClient).href);
const { StdioClientTransport } = await import(pathToFileURL(sdkStdio).href);

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const bin = join(root, "bin/hello-retrieval-mcp");
const query = process.argv[2] || "滴灌通";

if (!process.env.DATABASE_URL) {
  console.error("DATABASE_URL required");
  process.exit(1);
}

const transport = new StdioClientTransport({
  command: bin,
  args: [],
  env: { ...process.env },
  stderr: "inherit",
});

const client = new Client({ name: "p0-pi-mcp-path", version: "0.0.1" });
await client.connect(transport);

const tools = await client.listTools();
console.log(
  "==> tools:",
  tools.tools.map((t) => t.name).join(", "),
);

const result = await client.callTool({
  name: "lexical",
  arguments: { query, limit: 2 },
});
console.log("==> lexical result:");
console.log(JSON.stringify(result, null, 2));

await client.close();
console.log("==> OK (pi-mcp-adapter SDK path)");
