import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PaywallModal } from "../../components/billing/PaywallModal";

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

describe("PaywallModal", () => {
  it("renders title based on reason prop", () => {
    render(
      <PaywallModal
        reason="5h"
        locale="zh-CN"
        rolling5h={window5h}
        rolling7d={window7d}
        onContinueFree={vi.fn()}
      />,
    );
    expect(screen.getByText(/平台保护限速已触发/)).toBeTruthy();
  });

  it("embeds UsageMeter compact and routes upgrades to the canonical /pricing checkout", () => {
    render(
      <PaywallModal
        reason="5h"
        locale="zh-CN"
        rolling5h={window5h}
        rolling7d={window7d}
        onContinueFree={vi.fn()}
      />,
    );
    expect(screen.getAllByRole("progressbar").length).toBeGreaterThan(0);
    // PRODUCT_IA §4: paywall explains and links out; no second checkout here.
    expect(screen.getByTestId("paywall-view-plans").getAttribute("href")).toBe("/pricing");
  });

  it("calls onContinueFree when 稍后再说 clicked", () => {
    const onContinueFree = vi.fn();
    render(
      <PaywallModal
        reason="5h"
        locale="zh-CN"
        rolling5h={window5h}
        rolling7d={window7d}
        onContinueFree={onContinueFree}
      />,
    );
    screen.getByTestId("paywall-continue-free").click();
    expect(onContinueFree).toHaveBeenCalled();
  });
});
