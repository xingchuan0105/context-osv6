/** Platform protective rolling limits (hard-stop when no balance & no custom provider).
 * Not product primary entitlements under ADR-0010. */
export const PLAN_ROLLING_LIMITS: Record<string, { rolling5h: number; rolling7d: number }> = {
  free: { rolling5h: 100_000, rolling7d: 400_000 },
  plus: { rolling5h: 600_000, rolling7d: 4_000_000 },
  pro: { rolling5h: 2_500_000, rolling7d: 15_000_000 },
};

/** Plan margin multiplier M (wallet spend transparency; not a “tier benefit”). */
export const PLAN_MARGIN_MULTIPLIER: Record<string, number> = {
  free: 2.0,
  plus: 1.5,
  pro: 1.3,
};

/** ADR-0010 primary entitlement: shareable workspace slots. */
export const PLAN_SHARE_SLOTS: Record<string, number> = {
  free: 3,
  plus: 10,
  pro: 100,
  plus_annual: 10,
  pro_annual: 100,
};

export function getPlanRollingLimits(planId: string) {
  const base = planId.replace(/_annual$/, "");
  return PLAN_ROLLING_LIMITS[base] ?? PLAN_ROLLING_LIMITS[planId] ?? null;
}

export function getPlanMarginMultiplier(planId: string): number {
  const base = planId.replace(/_annual$/, "");
  return PLAN_MARGIN_MULTIPLIER[base] ?? PLAN_MARGIN_MULTIPLIER[planId] ?? 2.0;
}

export function getPlanShareSlots(planId: string): number | null {
  if (planId in PLAN_SHARE_SLOTS) {
    return PLAN_SHARE_SLOTS[planId];
  }
  const base = planId.replace(/_annual$/, "");
  return PLAN_SHARE_SLOTS[base] ?? null;
}

/** units → ≈ tokens under pure miss-input reference. */
export function tokensApproxFromUnits(units: number, marginMultiplier: number): number {
  const m = marginMultiplier > 0 ? marginMultiplier : 1;
  return Math.round((units / m) * 1000);
}
