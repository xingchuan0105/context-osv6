import { redirect } from "next/navigation";

import { billingApi } from "../../../lib/billing/api";
import type { BillingPlan } from "../../../lib/billing/api";
import { isPricingRevampEnabled } from "../../../lib/billing/featureFlag";
import { PricingPageClient } from "./pricing-page-client";

export const dynamic = "force-dynamic";

export default async function PricingPage() {
  const enabled = await isPricingRevampEnabled();
  if (!enabled) {
    redirect("/dashboard");
  }

  let plans: BillingPlan[] = [];
  try {
    const response = await billingApi.getPlans();
    plans = response.plans;
  } catch {
    plans = [];
  }

  return <PricingPageClient plans={plans} />;
}
