import type { Metadata } from "next";

import { LegalCenter } from "@/components/legal/legal-center";

export const metadata: Metadata = {
  title: "Legal center",
  description:
    "Context-OS legal center: terms of service, privacy policy, and open-source notices.",
  alternates: {
    canonical: "/en/legal",
    languages: {
      "zh-CN": "/legal",
      en: "/en/legal",
      "x-default": "/legal",
    },
  },
};

export default function EnLegalCenterPage() {
  return <LegalCenter locale="en" />;
}
