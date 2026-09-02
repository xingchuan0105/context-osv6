import type { Metadata } from "next";

import { ThirdPartyNotices } from "@/components/legal/third-party-notices";

export const metadata: Metadata = {
  title: "Full third-party notices",
  description:
    "Complete list of all third-party open-source components used by Context-OS and their licenses.",
  alternates: {
    canonical: "/en/legal/licenses/third-party",
    languages: {
      "zh-CN": "/legal/licenses/third-party",
      en: "/en/legal/licenses/third-party",
      "x-default": "/legal/licenses/third-party",
    },
  },
};

export default function EnThirdPartyNoticesPage() {
  return <ThirdPartyNotices locale="en" />;
}
