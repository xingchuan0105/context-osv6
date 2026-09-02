import type { Metadata } from "next";

import { DesktopPageClient } from "../../(marketing)/desktop/desktop-page-client";

export const metadata: Metadata = {
  title: "AI knowledge base desktop client · free download",
  description:
    "Context OS Windows desktop client: free to download and use, data stays on your machine, with MCP / CLI for local agents.",
  alternates: {
    canonical: "/en/desktop",
    languages: {
      "zh-CN": "/desktop",
      en: "/en/desktop",
      "x-default": "/desktop",
    },
  },
};

export default function EnDesktopProductPage() {
  return <DesktopPageClient locale="en" />;
}
