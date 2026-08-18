import { readFile } from "node:fs/promises";
import path from "node:path";
import type { Metadata } from "next";
import Link from "next/link";

import { renderAgentMarkdown } from "@/lib/api-access/render-agent-markdown";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Agent API 接入文档",
  description:
    "面向 Agent 的 Context OS API 接入说明：MCP / HTTP 调用、工作区密钥与权限边界。",
  alternates: { canonical: "/help/api-access/agents" },
};

const FALLBACK_DOC = `# API Access for Agents

Stable link: /help/api-access/agents

See product UI → Share workspace → API Access → Copy full agent pack.
`;

async function loadAgentDoc(): Promise<string> {
  try {
    const filePath = path.join(process.cwd(), "public/docs/api-access-for-agents.md");
    return await readFile(filePath, "utf8");
  } catch {
    return FALLBACK_DOC;
  }
}

/** Canonical agent-readable doc with real H2 structure for crawlers. */
export default async function HelpApiAccessAgentsPage() {
  const content = await loadAgentDoc();
  // Page already provides one H1; demote doc-level `#` title to avoid dual H1.
  const body = renderAgentMarkdown(content.replace(/^#\s+.+\n+/, ""));

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "48rem" }}>
        <header style={{ display: "grid", gap: "0.5rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            Agent-readable API access
          </p>
          <h1 className="app-page-title" style={{ margin: 0 }}>
            Context OS Agent API
          </h1>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
            Product: Context OS · Brand: ContextLM · Operator: Xing Chuan
          </p>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: 0 }}>
            Page copy last updated: 2026-08-12
          </p>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-button-secondary" href="/help/api-access">
              Human guide
            </Link>
            <Link className="app-button-secondary" href="/help/faq">
              FAQ
            </Link>
            <Link className="app-button-secondary" href="/help/compare">
              Compare
            </Link>
            <Link className="app-button-secondary" href="/help">
              Help
            </Link>
          </div>
        </header>
        <article
          className="app-surface-card"
          data-testid="agent-api-doc-body"
          style={{ margin: 0, overflow: "auto", padding: "1.25rem 1.35rem" }}
        >
          {body}
        </article>
      </div>
    </main>
  );
}
