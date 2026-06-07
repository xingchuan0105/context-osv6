import { buildApiUrl } from "../auth/client";

export async function isPricingRevampEnabled(): Promise<boolean> {
  if (process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED === "0") {
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
