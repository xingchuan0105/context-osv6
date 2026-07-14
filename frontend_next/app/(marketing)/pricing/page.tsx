"use client";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";
import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { MarketingShell } from "@/components/marketing-chrome";
import { PricingPageClient } from "./pricing-page-client";

export default function PricingPage() {
  return (
    <MarketingShell active="pricing">
      <PricingRevampGate redirectTo="/dashboard" requireUsageProbe={false}>
        <PricingPageClient />
      </PricingRevampGate>
      <LegalFooterLinks />
    </MarketingShell>
  );
}
