import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PricingCards } from "../../components/billing/PricingCards";
import type { BillingPlan } from "../../lib/billing/api";

const plans: BillingPlan[] = [
  {
    plan_id: "free",
    name: "Free",
    description: "体验",
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
    description: "深度研究",
    price_label: "¥49 / 月 · $9 / 月",
    price_label_cny: "¥49 / 月",
    price_label_usd: "$9 / 月",
    interval: "month",
    checkout_available: true,
    current: true,
    quotas: [],
  },
  {
    plan_id: "pro",
    name: "Pro",
    description: "重度无忧",
    price_label: "¥129 / 月 · $19 / 月",
    price_label_cny: "¥129 / 月",
    price_label_usd: "$19 / 月",
    interval: "month",
    checkout_available: true,
    current: false,
    quotas: [],
  },
];

describe("PricingCards", () => {
  it("renders three tier cards with prices", () => {
    render(<PricingCards plans={plans} highlightTier="plus" locale="zh-CN" onSelect={vi.fn()} />);
    expect(screen.getByText("Free")).toBeTruthy();
    expect(screen.getByText("Plus")).toBeTruthy();
    expect(screen.getByText("Pro")).toBeTruthy();
    expect(screen.getByText("¥49 / 月")).toBeTruthy();
    expect(screen.getByText("$9 / 月")).toBeTruthy();
  });

  it("shows 推荐 badge on highlighted tier", () => {
    render(<PricingCards plans={plans} highlightTier="plus" locale="zh-CN" onSelect={vi.fn()} />);
    expect(screen.getByText("推荐")).toBeTruthy();
  });

  it("marks current plan with disabled button", () => {
    render(<PricingCards plans={plans} highlightTier="plus" locale="zh-CN" onSelect={vi.fn()} />);
    const plusButton = screen.getByRole("button", { name: /当前方案/ }) as HTMLButtonElement;
    expect(plusButton.disabled).toBe(true);
  });

  it("calls onSelect with plan_id when clicking non-current tier", () => {
    const onSelect = vi.fn();
    render(<PricingCards plans={plans} highlightTier="plus" locale="zh-CN" onSelect={onSelect} />);
    screen.getByRole("button", { name: /升级 Pro/ }).click();
    expect(onSelect).toHaveBeenCalledWith("pro");
  });

  it("shows shareable workspace slots instead of 5h/7d token limits", () => {
    render(<PricingCards plans={plans} highlightTier="plus" locale="zh-CN" onSelect={vi.fn()} />);
    expect(screen.getByTestId("share-slots-free")).toHaveTextContent(/可分享工作区 3 个/);
    expect(screen.getByTestId("share-slots-plus")).toHaveTextContent(/可分享工作区 10 个/);
    expect(screen.getByTestId("share-slots-pro")).toHaveTextContent(/可分享工作区 100 个/);
    expect(screen.queryByText(/5 小时约/)).toBeNull();
    expect(screen.queryByText(/方案乘数 M=/)).toBeNull();
  });

  it("shows only the CNY label when priceProvider is alipay", () => {
    render(
      <PricingCards
        plans={plans}
        highlightTier="plus"
        locale="zh-CN"
        onSelect={vi.fn()}
        priceProvider="alipay"
      />,
    );
    expect(screen.getByTestId("price-plus")).toHaveTextContent("¥49 / 月");
    expect(screen.queryByText("$9 / 月")).toBeNull();
  });

  it("shows only the USD label when priceProvider is creem", () => {
    render(
      <PricingCards
        plans={plans}
        highlightTier="plus"
        locale="en"
        onSelect={vi.fn()}
        priceProvider="creem"
      />,
    );
    expect(screen.getByTestId("price-plus")).toHaveTextContent("$9 / 月");
    expect(screen.queryByText("¥49 / 月")).toBeNull();
  });
});
