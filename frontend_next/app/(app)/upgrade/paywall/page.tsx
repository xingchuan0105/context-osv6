import { redirect } from "next/navigation";

import { isPricingRevampEnabledSSR } from "../../../../lib/billing/featureFlag";
import { PaywallPageClient } from "./paywall-page-client";

export const dynamic = "force-dynamic";

type PaywallPageProps = {
  searchParams: Promise<{ reason?: string }>;
};

/** Env-only SSR gate; bucket check runs client-side via isPricingRevampEnabled(). */
export default async function PaywallPage({ searchParams }: PaywallPageProps) {
  if (!isPricingRevampEnabledSSR()) {
    redirect("/dashboard");
  }

  const params = await searchParams;
  const reason = params.reason === "7d" ? "7d" : "5h";

  return <PaywallPageClient reason={reason} />;
}
