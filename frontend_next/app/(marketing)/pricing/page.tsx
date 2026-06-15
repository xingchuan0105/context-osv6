"use client";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";
import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { PricingPageClient } from "./pricing-page-client";

export default function PricingPage() {
  return (
    <>
      <PricingRevampGate redirectTo="/dashboard" requireUsageProbe={false}>
        <PricingPageClient />
      </PricingRevampGate>
      <LegalFooterLinks />
    </>
  );
}
