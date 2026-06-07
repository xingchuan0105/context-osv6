import { redirect } from "next/navigation";

import { isPricingRevampEnabledSSR } from "../../../lib/billing/featureFlag";
import { PricingPageClient } from "./pricing-page-client";

export const dynamic = "force-dynamic";

export default async function PricingPage() {
  if (!isPricingRevampEnabledSSR()) {
    redirect("/dashboard");
  }

  return <PricingPageClient />;
}
