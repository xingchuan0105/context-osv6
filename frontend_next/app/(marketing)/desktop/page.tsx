import type { Metadata } from "next";

import { DesktopPageClient } from "./desktop-page-client";

export const metadata: Metadata = {
  title: "桌面客户端",
  description:
    "Context OS Windows 桌面客户端：免费下载使用，数据留在本机，支持 MCP / CLI 供桌面 Agent 调用。",
  alternates: { canonical: "/desktop" },
};

export default function DesktopProductPage() {
  return <DesktopPageClient />;
}
