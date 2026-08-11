import type { Metadata } from "next";

import HomeClient from "./home-client";

export const metadata: Metadata = {
  title: { absolute: "Context OS — 可分享的个人知识工作区" },
  description:
    "把分散的文档变成可检索、可分享、可被 AI 引用的知识工作区：文档入库与问答、MCP / API 外接 Agent、会员解锁可分享名额。",
  alternates: { canonical: "/" },
};

export default function HomePage() {
  return <HomeClient />;
}
