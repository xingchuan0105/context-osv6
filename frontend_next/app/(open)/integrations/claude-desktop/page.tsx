import type { Metadata } from "next";

import { IntegrationDocPage } from "@/components/integrations/integration-doc-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "把 Context OS 知识库接进 Claude Desktop（MCP）",
  description:
    "让 Claude Desktop 经 MCP 查询 Context OS 工作区：claude_desktop_config.json stdio 配置或远程连接器（/api/v1/mcp + Bearer 密钥），含验证步骤与权限边界。",
  alternates: {
    canonical: "/integrations/claude-desktop",
    languages: {
      "zh-CN": "/integrations/claude-desktop",
      "x-default": "/integrations/claude-desktop",
    },
  },
};

export default function IntegrationsClaudeDesktopRoute() {
  return <IntegrationDocPage slug="claude-desktop" />;
}
