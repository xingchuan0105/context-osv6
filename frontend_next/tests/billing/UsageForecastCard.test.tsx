import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { UsageForecastCard } from "../../components/billing/UsageForecastCard";

vi.mock("next-intl", () => ({
  useLocale: () => "zh-CN",
}));

describe("UsageForecastCard", () => {
  it("shows upgrade recommendation when flagged", () => {
    render(
      <UsageForecastCard
        suggestion_zh="按当前用量，本月建议升级到 Plus（7d 限额 4M）"
        suggestion_en="Based on current usage, upgrading to Plus is recommended"
        upgrade_recommended={true}
        projected_30d_tokens={3500000}
        current_limit_7d={400000}
      />,
    );
    expect(screen.getByText(/建议升级到 Plus/)).toBeTruthy();
  });

  it("shows no-upgrade message when under threshold", () => {
    render(
      <UsageForecastCard
        suggestion_zh="按当前用量，本月无需升级"
        suggestion_en="Based on current usage, no upgrade needed"
        upgrade_recommended={false}
        projected_30d_tokens={100000}
        current_limit_7d={400000}
      />,
    );
    expect(screen.getByText(/本月无需升级/)).toBeTruthy();
  });
});
