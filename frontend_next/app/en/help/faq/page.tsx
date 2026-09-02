import type { Metadata } from "next";

import { FaqPage } from "@/components/help/faq-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Context OS FAQ: MCP access, BYOK, share slots & pricing",
  description:
    "Context OS product FAQ: MCP / agent access, workspace key boundaries, share slots, BYOK, membership and wallet pricing.",
  alternates: {
    canonical: "/en/help/faq",
    languages: {
      "zh-CN": "/help/faq",
      en: "/en/help/faq",
      "x-default": "/help/faq",
    },
  },
};

export default function EnHelpFaqPage() {
  return <FaqPage locale="en" />;
}
