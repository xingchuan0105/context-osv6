import type { Metadata } from "next";

import { ApiAccessGuideClient } from "../../../(open)/help/api-access/api-access-guide-client";

export const metadata: Metadata = {
  title: "API access",
  description:
    "Context OS API access guide: each workspace manages its own keys; automated agents should use the agent docs and Agent Pack.",
  alternates: {
    canonical: "/en/help/api-access",
    languages: {
      "zh-CN": "/help/api-access",
      en: "/en/help/api-access",
      "x-default": "/help/api-access",
    },
  },
};

export default function EnHelpApiAccessPage() {
  return <ApiAccessGuideClient locale="en" />;
}
