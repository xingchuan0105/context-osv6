/** Rolling token limits per plan (spec §2.1). Used when API plan payloads omit window caps. */
export const PLAN_ROLLING_LIMITS: Record<string, { rolling5h: number; rolling7d: number }> = {
  free: { rolling5h: 100_000, rolling7d: 400_000 },
  plus: { rolling5h: 600_000, rolling7d: 4_000_000 },
  pro: { rolling5h: 2_500_000, rolling7d: 15_000_000 },
};

export function getPlanRollingLimits(planId: string) {
  return PLAN_ROLLING_LIMITS[planId] ?? null;
}
