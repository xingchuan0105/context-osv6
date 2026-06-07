import { buildApiUrl } from "../auth/client";

/**
 * Pricing revamp gate design (keep frontend aligned with backend PRICING_REVAMP_ROLLOUT):
 *
 * - Backend: hash-bucket rollout on user_id (0–99%) via PRICING_REVAMP_ROLLOUT; gated billing
 *   APIs return HTTP 200 with `{ ok: false, error: { code: "feature_disabled" } }`.
 * - Frontend env NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1 is a build-time prerequisite only.
 * - Anonymous /pricing: env gate is enough (marketing page; no usage API). SSR redirects when env=0.
 * - Authenticated surfaces (usage, paywall, workspace): MUST call isPricingRevampEnabled() which
 *   checks env AND a successful /billing/usage/window probe so UI matches the user's bucket.
 */
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

/** Sync env prerequisite only — do not use alone on authenticated billing UI. */
export function isPricingRevampEnabledClient(): boolean {
  return isPricingRevampEnabledSSR();
}

type UsageWindowProbeEnvelope = {
  ok?: boolean;
  error?: { code?: string; message?: string } | null;
};

/** Canonical authenticated check: env prerequisite + usage/window probe (bucket-aware). */
export async function isPricingRevampEnabled(): Promise<boolean> {
  if (!isPricingRevampEnabledSSR()) {
    return false;
  }

  try {
    const response = await fetch(buildApiUrl("/api/v1/billing/usage/window"), {
      credentials: "include",
      cache: "no-store",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      return false;
    }
    const envelope = (await response.json()) as UsageWindowProbeEnvelope;
    return envelope.ok === true;
  } catch {
    return false;
  }
}

export function isPricingRevampFeatureDisabledError(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }
  return error.message.includes("feature_disabled") || error.message.includes("not yet available");
}
