import { redirect } from "next/navigation";

import { isPricingRevampEnabledSSR } from "../../../../lib/billing/featureFlag";
import { UsageDashboardClient } from "./usage-dashboard-client";

export const dynamic = "force-dynamic";

export default async function UsagePage() {
  if (!isPricingRevampEnabledSSR()) {
    redirect("/settings");
  }

  return <UsageDashboardClient />;
}
