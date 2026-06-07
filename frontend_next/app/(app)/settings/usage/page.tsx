import { redirect } from "next/navigation";

import { isPricingRevampEnabled } from "../../../lib/billing/featureFlag";
import { UsageDashboardClient } from "./usage-dashboard-client";

export const dynamic = "force-dynamic";

export default async function UsagePage() {
  const enabled = await isPricingRevampEnabled();
  if (!enabled) {
    redirect("/settings");
  }

  return <UsageDashboardClient />;
}
