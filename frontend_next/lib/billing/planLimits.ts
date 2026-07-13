/** Product-facing ≈ tokens per plan (design §5.1). Used when API omits approx fields. */
export const PLAN_ROLLING_LIMITS: Record<string, { rolling5h: number; rolling7d: number }> = {
  free: { rolling5h: 100_000, rolling7d: 400_000 },
  plus: { rolling5h: 600_000, rolling7d: 4_000_000 },
  pro: { rolling5h: 2_500_000, rolling7d: 15_000_000 },
};

/** Plan margin multiplier M (transparent; free 2.0 / plus 1.5 / pro 1.3). */
export const PLAN_MARGIN_MULTIPLIER: Record<string, number> = {
  free: 2.0,
  plus: 1.5,
  pro: 1.3,
};

export function getPlanRollingLimits(planId: string) {
  return PLAN_ROLLING_LIMITS[planId] ?? null;
}

export function getPlanMarginMultiplier(planId: string): number {
  return PLAN_MARGIN_MULTIPLIER[planId] ?? 2.0;
}

/** units → ≈ tokens under pure miss-input reference. */
export function tokensApproxFromUnits(units: number, marginMultiplier: number): number {
  const m = marginMultiplier > 0 ? marginMultiplier : 1;
  return Math.round((units / m) * 1000);
}
