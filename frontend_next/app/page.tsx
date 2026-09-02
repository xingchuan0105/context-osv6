import type { Metadata } from "next";

import { OrganizationJsonLd } from "../components/organization-jsonld";
import { SoftwareApplicationJsonLd } from "../components/software-application-jsonld";
import HomeClient from "./home-client";

export const metadata: Metadata = {
  title: { absolute: "Context OS（ContextLM 旗下）— 可本地部署的个人 AI 知识库" },
  description:
    "可本地部署的个人 AI 知识库：文档入库即问答，答案引用到原文；支持 MCP / API 接入外接 Agent，可把知识库分享给他人；桌面客户端免费。",
  alternates: {
    canonical: "/",
    languages: {
      "zh-CN": "/",
      en: "/en",
      "x-default": "/",
    },
  },
};

export default function HomePage() {
  return (
    <>
      <OrganizationJsonLd />
      <SoftwareApplicationJsonLd />
      <HomeClient />
    </>
  );
}
