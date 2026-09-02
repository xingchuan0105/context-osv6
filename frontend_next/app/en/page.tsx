import type { Metadata } from "next";

import HomeClient from "../home-client";

export const metadata: Metadata = {
  title: { absolute: "Context OS by ContextLM — a locally deployable personal AI knowledge base" },
  description:
    "A locally deployable personal AI knowledge base: ingest documents and ask questions with answers cited to source; connect external agents via MCP / API; share workspaces with others; free desktop client.",
  alternates: {
    canonical: "/en",
    languages: {
      "zh-CN": "/",
      en: "/en",
      "x-default": "/",
    },
  },
};

export default function EnHomePage() {
  return <HomeClient locale="en" />;
}
