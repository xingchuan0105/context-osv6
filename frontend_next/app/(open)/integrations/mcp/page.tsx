import type { Metadata } from "next";

import { IntegrationDocPage } from "@/components/integrations/integration-doc-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Context OS MCP 接入总入口：端点、工具目录与边界",
  description:
    "Context OS 每个工作区一个 MCP HTTP 端点（/api/v1/mcp，JSON-RPC）：连接参数、workspace.* 工具目录、调用示例、本地 stdio 包装器与权限边界。",
  alternates: {
    canonical: "/integrations/mcp",
    languages: {
      "zh-CN": "/integrations/mcp",
      "x-default": "/integrations/mcp",
    },
  },
};

export default function IntegrationsMcpRoute() {
  return <IntegrationDocPage slug="mcp" />;
}
