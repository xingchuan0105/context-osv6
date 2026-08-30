import type { Metadata } from "next";

import HomeClient from "./home-client";

export const metadata: Metadata = {
  title: { absolute: "Context OS — 可本地部署的个人 AI 知识库" },
  description:
    "可本地部署的个人 AI 知识库：文档入库即问答，答案引用到原文；支持 MCP / API 接入外接 Agent，可把知识库分享给他人；桌面客户端免费。",
  alternates: { canonical: "/" },
};

export default function HomePage() {
  return <HomeClient />;
}
