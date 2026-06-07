import { buildApiUrl } from "../auth/client";

/** SSR / marketing pages: env-only gate aligned with backend PRICING_REVAMP_ROLLOUT default (0). */
export function isPricingRevampEnabledSSR(): boolean {
  const flag = process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;
  if (flag === "0") {
    return false;
  }
  if (flag === "1") {
    return true;
  }
  return false;
}

/** Client components: sync check without auth probe. */
export function isPricingRevampEnabledClient(): boolean {
  return isPricingRevampEnabledSSR();
}

/** Authenticated client contexts: verify the usage window API is reachable. */
export async function isPricingRevampEnabled(): Promise<boolean> {
  if (!isPricingRevampEnabledSSR()) {
    return false;
  }

  try {
    const response = await fetch(buildApiUrl("/api/v1/billing/usage/window"), {
      credentials: "include",
      cache: "no-store",
    });
    return response.ok;
  } catch {
    return false;
  }
}
