import type { BillingPlan } from "./api";

/** Static catalog for anonymous /pricing visitors when /billing/plans requires auth. */
export const MARKETING_BILLING_PLANS: BillingPlan[] = [
  {
    plan_id: "free",
    name: "Free",
    description: "Client free · private use free · 3 shareable workspaces",
    price_label: "¥0",
    price_label_cny: "¥0",
    price_label_usd: "$0",
    interval: "month",
    checkout_available: false,
    current: false,
    quotas: [],
  },
  {
    plan_id: "plus",
    name: "Plus",
    description: "10 shareable workspaces",
    price_label: "¥49 / 月 · $9 / 月",
    price_label_cny: "¥49 / 月",
    price_label_usd: "$9 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
  {
    plan_id: "pro",
    name: "Pro",
    description: "100 shareable workspaces",
    price_label: "¥129 / 月 · $19 / 月",
    price_label_cny: "¥129 / 月",
    price_label_usd: "$19 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
  {
    plan_id: "plus_annual",
    name: "Plus",
    description: "10 shareable workspaces · billed yearly (~10× monthly)",
    price_label: "¥490 / 年 · $90 / 年",
    price_label_cny: "¥490 / 年",
    price_label_usd: "$90 / 年",
    interval: "year",
    checkout_available: true,
    current: false,
    quotas: [],
  },
  {
    plan_id: "pro_annual",
    name: "Pro",
    description: "100 shareable workspaces · billed yearly (~10× monthly)",
    price_label: "¥1290 / 年 · $190 / 年",
    price_label_cny: "¥1290 / 年",
    price_label_usd: "$190 / 年",
    interval: "year",
    checkout_available: true,
    current: false,
    quotas: [],
  },
];

/** Plans shown for a billing interval toggle (Free always included). */
export function plansForInterval(
  plans: BillingPlan[],
  interval: "month" | "year",
): BillingPlan[] {
  const free = plans.find((p) => p.plan_id === "free");
  const paid = plans.filter((p) => {
    if (p.plan_id === "free") return false;
    if (interval === "year") {
      return p.plan_id.endsWith("_annual") || p.interval === "year";
    }
    return (
      !p.plan_id.endsWith("_annual") &&
      (p.interval === "month" || p.interval === "monthly" || !p.interval)
    );
  });
  return free ? [free, ...paid] : paid;
}
