import type { Metadata } from "next";

import { LicensesSummary } from "@/components/legal/licenses-summary";

export const metadata: Metadata = {
  title: "Open-source notices",
  description:
    "Open-source components used by Context-OS and a summary of their licenses.",
  alternates: {
    canonical: "/en/legal/licenses",
    languages: {
      "zh-CN": "/legal/licenses",
      en: "/en/legal/licenses",
      "x-default": "/legal/licenses",
    },
  },
};

export default function EnLicensesSummaryPage() {
  return <LicensesSummary locale="en" />;
}
