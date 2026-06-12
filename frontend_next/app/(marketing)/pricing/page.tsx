"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

import { isPricingRevampEnabled, isPricingRevampEnabledSSR } from "../../../lib/billing/featureFlag";
import { PricingPageClient } from "./pricing-page-client";

export default function PricingPage() {
  const router = useRouter();

  useEffect(() => {
    isPricingRevampEnabled().then((enabled) => {
      if (!enabled) {
        router.replace("/dashboard");
      }
    });
  }, [router]);

  if (!isPricingRevampEnabledSSR()) {
    return null;
  }

  return <PricingPageClient />;
}
