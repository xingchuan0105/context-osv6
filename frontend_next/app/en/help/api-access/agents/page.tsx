import type { Metadata } from "next";

import { AgentApiPage } from "@/components/help/agent-api-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Agent API access docs",
  description:
    "Agent-facing Context OS API access guide: MCP / HTTP calls, workspace keys, and permission boundaries.",
  alternates: {
    canonical: "/en/help/api-access/agents",
    languages: {
      "zh-CN": "/help/api-access/agents",
      en: "/en/help/api-access/agents",
      "x-default": "/help/api-access/agents",
    },
  },
};

export default function EnHelpApiAccessAgentsPage() {
  return <AgentApiPage />;
}
