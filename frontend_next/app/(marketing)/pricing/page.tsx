import type { Metadata } from "next";

import { PricingRevampGate } from "@/components/billing/PricingRevampGate";
import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { MarketingShell } from "@/components/marketing-chrome";
import { PricingPageClient } from "./pricing-page-client";

export const metadata: Metadata = {
  title: "定价",
  description:
    "Context OS 会员档位与本页充值：免费档即可建仓，升级解锁更多可分享名额。",
  alternates: { canonical: "/pricing" },
};

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
