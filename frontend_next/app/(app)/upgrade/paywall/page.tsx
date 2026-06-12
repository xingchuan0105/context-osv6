"use client";

import { useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";

import { isPricingRevampEnabled, isPricingRevampEnabledSSR } from "../../../../lib/billing/featureFlag";
import { PaywallPageClient } from "./paywall-page-client";

export default function PaywallPage() {
  const router = useRouter();
  const searchParams = useSearchParams();

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

  const reason = searchParams.get("reason") === "7d" ? "7d" : "5h";

  return <PaywallPageClient reason={reason} />;
}
