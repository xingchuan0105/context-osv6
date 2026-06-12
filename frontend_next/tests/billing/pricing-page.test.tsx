import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

import { PricingPageClient } from "../../app/(marketing)/pricing/pricing-page-client";

const mockPlans = vi.hoisted(() => globalThis.__mockProviders.createPricingPageMockPlans());



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
