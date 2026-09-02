import type { Metadata } from "next";

import { FaqPage } from "@/components/help/faq-page";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "Context OS 常见问题：MCP 接入、BYOK、分享名额与定价",
  description:
    "Context OS 产品 FAQ：MCP / Agent 接入、工作区密钥边界、可分享名额、BYOK、会员与余额定价。",
  alternates: {
    canonical: "/help/faq",
    languages: {
      "zh-CN": "/help/faq",
      en: "/en/help/faq",
      "x-default": "/help/faq",
    },
  },
};

export default function HelpFaqPage() {
  return <FaqPage locale="zh-CN" />;
}
