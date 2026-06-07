import { redirect } from "next/navigation";

import { isPricingRevampEnabledSSR } from "../../../../lib/billing/featureFlag";
import { UsageDashboardClient } from "./usage-dashboard-client";

export const dynamic = "force-dynamic";

/** Env-only SSR gate; bucket check runs client-side via isPricingRevampEnabled(). */
export default async function UsagePage() {
  if (!isPricingRevampEnabledSSR()) {
    redirect("/settings");
  }

  return <UsageDashboardClient />;
}
