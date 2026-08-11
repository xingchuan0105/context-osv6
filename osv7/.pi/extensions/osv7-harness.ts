/**
 * osv7 harness bridge for pi: first-class tools → retrieval-cli (same Service as MCP).
 * Product main-agent path; external agents still use retrieval-mcp.
 */
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";
import { spawnSync } from "node:child_process";
import { join } from "node:path";

function cliBin(): string {
  return (
    process.env.OSV7_RETRIEVAL_CLI ||
    join(process.cwd(), "bin", "retrieval-cli")
  );
}

function runCli(args: string[]): { ok: boolean; text: string } {
  const r = spawnSync(cliBin(), args, {
    encoding: "utf8",
    env: process.env,
    maxBuffer: 4 * 1024 * 1024,
  });
  const text = `${r.stdout || ""}${r.stderr || ""}`.trim();
  if (r.status === 0) {
    return { ok: true, text: r.stdout?.trim() || text };
  }
  return { ok: false, text: text || `exit ${r.status}` };
}

export default function (pi: ExtensionAPI) {
  const workspaceDefault = process.env.OSV7_WORKSPACE_ID || "";

  pi.on("session_start", async (_e, ctx) => {
    if (workspaceDefault) {
      pi.sendMessage(
        {
          customType: "osv7-harness",
          content:
            `检索环境观察：本会话默认 workspace_id=${workspaceDefault}。` +
            `语料检索经 set_query_card / lexical / dense / grep；无卡不检索。` +
            `web 不在 harness 内。`,
          display: true,
        },
        { deliverAs: "nextTurn" },
      );
    }
  });

  pi.registerTool({
    name: "set_query_card",
    label: "Set query card",
    description:
      "Install task-level query card before corpus retrieval. Requires workspace_id.",
    parameters: Type.Object({
      workspace_id: Type.Optional(Type.String()),
      required_actions: Type.Optional(
        Type.Array(Type.String({ description: "dense|lexical|grep" })),
      ),
      question_type: Type.Optional(Type.String()),
    }),
    async execute(_id, params) {
      const ws = (params.workspace_id as string) || workspaceDefault;
      const actions = (params.required_actions as string[] | undefined) || [
        "lexical",
        "dense",
      ];
      const qtype = (params.question_type as string) || "rag_fact";
      const r = runCli([
        "set-card",
        "--workspace",
        ws,
        "--actions",
        actions.join(","),
        "--type",
        qtype,
      ]);
      return {
        content: [{ type: "text", text: r.text }],
        details: { ok: r.ok },
        isError: !r.ok,
      };
    },
  });

  pi.registerTool({
    name: "lexical",
    label: "Lexical search",
    description: "Lexical/FTS corpus search. Requires active query card.",
    parameters: Type.Object({
      query: Type.String(),
      limit: Type.Optional(Type.Number()),
    }),
    async execute(_id, params) {
      const args = ["lexical", "--query", String(params.query)];
      if (params.limit) args.push("--limit", String(params.limit));
      const r = runCli(args);
      return {
        content: [{ type: "text", text: r.text }],
        details: { ok: r.ok },
        isError: !r.ok,
      };
    },
  });

  pi.registerTool({
    name: "dense",
    label: "Dense search",
    description: "Dense vector corpus search. Requires active query card + embedding.",
    parameters: Type.Object({
      query: Type.String(),
      limit: Type.Optional(Type.Number()),
    }),
    async execute(_id, params) {
      const args = ["dense", "--query", String(params.query)];
      if (params.limit) args.push("--limit", String(params.limit));
      const r = runCli(args);
      return {
        content: [{ type: "text", text: r.text }],
        details: { ok: r.ok },
        isError: !r.ok,
      };
    },
  });

  pi.registerTool({
    name: "grep",
    label: "Grep corpus",
    description: "Literal substring grep over chunks. Requires active query card.",
    parameters: Type.Object({
      pattern: Type.String(),
      limit: Type.Optional(Type.Number()),
    }),
    async execute(_id, params) {
      const args = ["grep", "--pattern", String(params.pattern)];
      if (params.limit) args.push("--limit", String(params.limit));
      const r = runCli(args);
      return {
        content: [{ type: "text", text: r.text }],
        details: { ok: r.ok },
        isError: !r.ok,
      };
    },
  });
}
