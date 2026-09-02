import type { Metadata } from "next";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";
import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { MarketingShell } from "@/components/marketing-chrome";
import { PricingPageClient } from "../../(marketing)/pricing/pricing-page-client";

export const metadata: Metadata = {
  title: "Pricing",
  description:
    "Context OS membership tiers and on-page top-up: start free, upgrade to unlock more share slots.",
  alternates: {
    canonical: "/en/pricing",
    languages: {
      "zh-CN": "/pricing",
      en: "/en/pricing",
      "x-default": "/pricing",
    },
  },
};

export default function EnPricingPage() {
  return (
    <MarketingShell active="pricing" locale="en">
      <PricingRevampGate redirectTo="/dashboard" requireUsageProbe={false}>
        <PricingPageClient locale="en" />
      </PricingRevampGate>
      <LegalFooterLinks />
    </MarketingShell>
  );
}
