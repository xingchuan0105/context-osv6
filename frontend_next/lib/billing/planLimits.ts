/** Residual rolling token display map only. ADR-0010: 0 = unlimited (not product rights).
 * Spend gates are wallet / BYOK, not free-plan unit caps. */
export const PLAN_ROLLING_LIMITS: Record<string, { rolling5h: number; rolling7d: number }> = {
  free: { rolling5h: 0, rolling7d: 0 },
  plus: { rolling5h: 0, rolling7d: 0 },
  pro: { rolling5h: 0, rolling7d: 0 },
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
