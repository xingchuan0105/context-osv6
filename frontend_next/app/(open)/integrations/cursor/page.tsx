import type { Metadata } from "next";

import { IntegrationDocPage } from "@/components/integrations/integration-doc-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "把 Context OS 知识库接进 Cursor（MCP）",
  description:
    "两条可复现路径把 Context OS 工作区接进 Cursor：本地 stdio（context-os-mcp）与云端 MCP HTTP（/api/v1/mcp + Bearer 密钥）；含验证步骤与权限边界。",
  alternates: {
    canonical: "/integrations/cursor",
    languages: {
      "zh-CN": "/integrations/cursor",
      "x-default": "/integrations/cursor",
    },
  },
};

export default function IntegrationsCursorRoute() {
  return <IntegrationDocPage slug="cursor" />;
}
