import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PaywallModal } from "../../components/billing/PaywallModal";
import type { BillingPlan } from "../../lib/billing/api";

const window5h = {
  used: 100000,
  limit: 100000,
  percentage: 100,
  reset_at: "2099-12-31T00:00:00Z",
};
const window7d = {
  used: 100000,
  limit: 400000,
  percentage: 25,
  reset_at: "2099-12-31T00:00:00Z",
};

const plans: BillingPlan[] = [
  {
    plan_id: "free",
    name: "Free",
    description: "",
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
    description: "",
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
    description: "",
    price_label: "¥129 / 月 · $19 / 月",
    price_label_cny: "¥129 / 月",
    price_label_usd: "$19 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
];

describe("PaywallModal", () => {
  it("renders title based on reason prop", () => {
    render(
      <PaywallModal
        reason="5h"
        plans={plans}
        rolling5h={window5h}
        rolling7d={window7d}
        onSelect={vi.fn()}
        onContinueFree={vi.fn()}
      />,
    );
    expect(screen.getByText(/5h 用量已达上限/)).toBeTruthy();
  });

  it("embeds UsageMeter compact + PricingCards compact", () => {
    render(
      <PaywallModal
        reason="5h"
        plans={plans}
        rolling5h={window5h}
        rolling7d={window7d}
        onSelect={vi.fn()}
        onContinueFree={vi.fn()}
      />,
    );
    expect(screen.getAllByRole("progressbar").length).toBeGreaterThan(0);
  });

  it("calls onContinueFree when 继续 Free clicked", () => {
    const onContinueFree = vi.fn();
    render(
      <PaywallModal
        reason="5h"
        plans={plans}
        rolling5h={window5h}
        rolling7d={window7d}
        onSelect={vi.fn()}
        onContinueFree={onContinueFree}
      />,
    );
    screen.getByTestId("paywall-continue-free").click();
    expect(onContinueFree).toHaveBeenCalled();
  });
});
