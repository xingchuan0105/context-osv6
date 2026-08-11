import type { Metadata } from "next";

import { ApiAccessGuideClient } from "./api-access-guide-client";

export const metadata: Metadata = {
  title: "API 访问",
  description:
    "Context OS API 接入说明：每个工作区单独管理密钥；自动化代理（Agent）请使用 agent 文档与 Agent Pack。",
  alternates: { canonical: "/help/api-access" },
};

export default function HelpApiAccessPage() {
  return <ApiAccessGuideClient />;
}
