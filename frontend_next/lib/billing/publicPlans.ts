import type { BillingPlan } from "./api";

/** Static catalog for anonymous /pricing visitors when /billing/plans requires auth. */
export const MARKETING_BILLING_PLANS: BillingPlan[] = [
  {
    plan_id: "free",
    name: "Free",
    description: "Starter plan for smaller personal notebooks and trial usage.",
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
    description: "Daily quotas for active document ingestion and chat workflows.",
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
    description: "Unlimited quota posture for heavier workloads.",
    price_label: "¥129 / 月 · $19 / 月",
    price_label_cny: "¥129 / 月",
    price_label_usd: "$19 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
];
