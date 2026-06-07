import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

import { PricingPageClient } from "../../app/(marketing)/pricing/pricing-page-client";
import type { BillingPlan } from "../../lib/billing/api";

const mockPlans = vi.hoisted((): BillingPlan[] => [
  {
    plan_id: "free",
    name: "Free",
    price_label_cny: "¥0",
    price_label_usd: "$0",
    description: "体验",
    price_label: "¥0",
    interval: "month",
    checkout_available: false,
    current: false,
    quotas: [],
  },
  {
    plan_id: "plus",
    name: "Plus",
    price_label_cny: "¥49 / 月",
    price_label_usd: "$9 / 月",
    description: "深度研究",
    price_label: "¥49 / 月 · $9 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
  {
    plan_id: "pro",
    name: "Pro",
    price_label_cny: "¥129 / 月",
    price_label_usd: "$19 / 月",
    description: "重度无忧",
    price_label: "¥129 / 月 · $19 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
]);

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({ token: "token-1", user: { id: "u1" } }),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const }),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock("../../lib/settings/client", () => ({
  createCheckoutSession: vi.fn(),
}));

vi.mock("../../lib/billing/api", () => ({
  billingApi: {
    getPlans: vi.fn().mockResolvedValue({ plans: mockPlans, current_plan_id: "free" }),
  },
}));

describe("PricingPage", () => {
  it("renders title + 3 plan cards + FAQ", async () => {
    render(<PricingPageClient />);
    expect(await screen.findByText(/选择适合你的方案/)).toBeTruthy();
    expect(screen.getByText("Plus")).toBeTruthy();
    expect(screen.getByText(/token 用量怎么算/)).toBeTruthy();
  });
});
