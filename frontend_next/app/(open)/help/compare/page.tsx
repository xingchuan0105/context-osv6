import type { Metadata } from "next";

import { ComparePage } from "@/components/help/compare-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "AI 知识库工具对比",
  description:
    "个人 AI 知识库怎么选？Context OS 与笔记内置 AI、第二大脑应用、通用 RAG 套件的中立对比：适用场景、分享、外接 Agent，不编造竞品数据。",
  alternates: {
    canonical: "/help/compare",
    languages: {
      "zh-CN": "/help/compare",
      en: "/en/help/compare",
      "x-default": "/help/compare",
    },
  },
};

export default function HelpComparePage() {
  return <ComparePage locale="zh-CN" />;
}
