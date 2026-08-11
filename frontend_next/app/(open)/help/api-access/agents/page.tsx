import { readFile } from "node:fs/promises";
import path from "node:path";
import type { Metadata } from "next";
import Link from "next/link";

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

/** Canonical agent-readable doc. Prefer this over public/*.md (404 on some deploys). */
export default async function HelpApiAccessAgentsPage() {
  const content = await loadAgentDoc();

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "48rem" }}>
        <header style={{ display: "grid", gap: "0.5rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            Agent-readable API access
          </p>
          <h1 className="app-page-title" style={{ margin: 0 }}>
            /help/api-access/agents
          </h1>
          <div className="app-button-row">
            <Link className="app-button-secondary" href="/help/api-access">
              Human guide
            </Link>
            <Link className="app-button-secondary" href="/help">
              Help
            </Link>
          </div>
        </header>
        <article className="app-surface-card" style={{ margin: 0, overflow: "auto", padding: "1.25rem" }}>
          <pre
            data-testid="agent-api-doc-body"
            style={{
              fontFamily: "var(--font-mono, ui-monospace, monospace)",
              fontSize: "0.875rem",
              lineHeight: 1.55,
              margin: 0,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {content}
          </pre>
        </article>
      </div>
    </main>
  );
}
