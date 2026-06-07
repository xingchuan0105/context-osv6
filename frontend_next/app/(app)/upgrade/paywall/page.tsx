import { redirect } from "next/navigation";

import { isPricingRevampEnabled } from "../../../../lib/billing/featureFlag";
import { PaywallPageClient } from "./paywall-page-client";

export const dynamic = "force-dynamic";

type PaywallPageProps = {
  searchParams: Promise<{ reason?: string }>;
};

export default async function PaywallPage({ searchParams }: PaywallPageProps) {
  const enabled = await isPricingRevampEnabled();
  if (!enabled) {
    redirect("/dashboard");
  }

  const params = await searchParams;
  const reason = params.reason === "7d" ? "7d" : "5h";

  return <PaywallPageClient reason={reason} />;
}
