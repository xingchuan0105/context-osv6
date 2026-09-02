import type { Metadata } from "next";

import { ComparePage } from "@/components/help/compare-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "AI knowledge base comparison",
  description:
    "How to choose a personal AI knowledge base? A neutral comparison of Context OS vs notes AI, second-brain apps, and general RAG stacks: use cases, sharing, external agents — no fabricated competitor data.",
  alternates: {
    canonical: "/en/help/compare",
    languages: {
      "zh-CN": "/help/compare",
      en: "/en/help/compare",
      "x-default": "/help/compare",
    },
  },
};

export default function EnHelpComparePage() {
  return <ComparePage locale="en" />;
}
