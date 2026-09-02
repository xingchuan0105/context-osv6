import type { Metadata } from "next";

import { AgentApiPage } from "@/components/help/agent-api-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Agent API 接入文档",
  description:
    "面向 Agent 的 Context OS API 接入说明：MCP / HTTP 调用、工作区密钥与权限边界。",
  alternates: {
    canonical: "/help/api-access/agents",
    languages: {
      "zh-CN": "/help/api-access/agents",
      en: "/en/help/api-access/agents",
      "x-default": "/help/api-access/agents",
    },
  },
};

export default function HelpApiAccessAgentsPage() {
  return <AgentApiPage />;
}
