import { redirect } from "next/navigation";

import { isPricingRevampEnabledSSR } from "../../../lib/billing/featureFlag";
import { PricingPageClient } from "./pricing-page-client";

export const dynamic = "force-dynamic";

/** Env-only SSR gate; marketing page does not probe usage API (see featureFlag.ts). */
export default async function PricingPage() {
  if (!isPricingRevampEnabledSSR()) {
    redirect("/dashboard");
  }

  return <PricingPageClient />;
}
