"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

import { isPricingRevampEnabled, isPricingRevampEnabledSSR } from "../../../../lib/billing/featureFlag";
import { UsageDashboardClient } from "./usage-dashboard-client";

export default function UsagePage() {
  const router = useRouter();

  useEffect(() => {
    isPricingRevampEnabled().then((enabled) => {
      if (!enabled) {
        router.replace("/settings");
      }
    });
  }, [router]);

  if (!isPricingRevampEnabledSSR()) {
    return null;
  }

  return <UsageDashboardClient />;
}
