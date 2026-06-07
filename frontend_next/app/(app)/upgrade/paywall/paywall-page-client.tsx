"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { PaywallModal } from "../../../../components/billing/PaywallModal";
import { billingApi } from "../../../../lib/billing/api";
import type { BillingPlan, UsageWindowResponse } from "../../../../lib/billing/api";
import { createCheckoutSession } from "../../../../lib/settings/client";
import { useAuth } from "../../../../lib/auth/context";

export function PaywallPageClient({ reason }: { reason: "5h" | "7d" }) {
  const auth = useAuth();
  const router = useRouter();
  const [window, setWindow] = useState<UsageWindowResponse | null>(null);
  const [plans, setPlans] = useState<BillingPlan[]>([]);

  useEffect(() => {
    void Promise.all([billingApi.getUsageWindow(), billingApi.getPlans()]).then(
      ([windowData, plansData]) => {
        setWindow(windowData);
        setPlans(plansData.plans);
      },
    );
  }, []);

  if (!window) {
    return null;
  }

  async function handleSelect(planId: string) {
    if (planId === "free" || !auth.token) {
      return;
    }
    const checkout = await createCheckoutSession(auth.token, { plan_id: planId });
    if (checkout.url) {
      router.push(checkout.url);
    }
  }

  function handleContinueFree() {
    router.push("/dashboard");
  }

  return (
    <PaywallModal
      reason={reason}
      plans={plans}
      rolling5h={window.rolling_5h}
      rolling7d={window.rolling_7d}
      onSelect={handleSelect}
      onContinueFree={handleContinueFree}
    />
  );
}
