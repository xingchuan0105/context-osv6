import type { Metadata } from "next";

import { IntegrationIndexPage } from "@/components/integrations/integration-index-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "把 Context OS 知识库接进 Cursor / Claude（MCP 集成）",
  description:
    "Context OS 集成承接：按客户端给出把工作区知识库接进 Cursor、Claude Desktop 与任意 MCP 客户端的可复现步骤；接入协议事实源见 Agent API 文档。",
  alternates: {
    canonical: "/integrations",
    languages: {
      "zh-CN": "/integrations",
      "x-default": "/integrations",
    },
  },
};

export default function IntegrationsIndexRoute() {
  return <IntegrationIndexPage />;
}
