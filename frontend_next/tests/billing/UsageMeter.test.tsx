import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UsageMeter } from "../../components/billing/UsageMeter";

describe("UsageMeter", () => {
  it("renders full variant with both 5h and 7d buckets as approx tokens", () => {
    render(
      <UsageMeter
        variant="full"
        locale="zh-CN"
        planId="free"
        marginMultiplier={2.0}
        rolling5h={{
          used: 160,
          limit: 200,
          used_tokens_approx: 80_000,
          limit_tokens_approx: 100_000,
          percentage: 80,
          reset_at: "2026-06-07T20:00:00Z",
        }}
        rolling7d={{
          used: 400,
          limit: 800,
          used_tokens_approx: 200_000,
          limit_tokens_approx: 400_000,
          percentage: 50,
          reset_at: "2026-06-10T00:00:00Z",
        }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: false, rolling_7d: false }}
      />,
    );
    expect(screen.getByText(/5 小时窗口/)).toBeTruthy();
    expect(screen.getByText(/7 天窗口/)).toBeTruthy();
    expect(screen.getByText((_, el) => el?.textContent?.trim() === "80.0K")).toBeTruthy();
    expect(screen.getByTestId("usage-margin-note").textContent).toMatch(/M=2/);
  });

  it("renders compact variant with just progress bars", () => {
    render(
      <UsageMeter
        variant="compact"
        locale="zh-CN"
        planId="free"
        rolling5h={{
          used: 200,
          limit: 200,
          used_tokens_approx: 100_000,
          limit_tokens_approx: 100_000,
          percentage: 100,
          reset_at: "2026-06-07T20:00:00Z",
        }}
        rolling7d={{
          used: 200,
          limit: 800,
          used_tokens_approx: 100_000,
          limit_tokens_approx: 400_000,
          percentage: 25,
          reset_at: "2026-06-10T00:00:00Z",
        }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: true, rolling_7d: false }}
      />,
    );
    expect(screen.queryByText(/5 小时窗口/)).toBeNull();
    expect(screen.getAllByRole("progressbar").length).toBe(2);
  });

  it("shows warning text when soft limit hit", () => {
    render(
      <UsageMeter
        variant="full"
        locale="zh-CN"
        planId="free"
        marginMultiplier={2}
        rolling5h={{
          used: 160,
          limit: 200,
          used_tokens_approx: 80_000,
          limit_tokens_approx: 100_000,
          percentage: 80,
          reset_at: "2026-06-07T20:00:00Z",
        }}
        rolling7d={{
          used: 200,
          limit: 800,
          used_tokens_approx: 100_000,
          limit_tokens_approx: 400_000,
          percentage: 25,
          reset_at: "2026-06-10T00:00:00Z",
        }}
        softLimitHit={{ rolling_5h: true, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: false, rolling_7d: false }}
      />,
    );
    expect(screen.getByText(/已超过软上限/)).toBeTruthy();
  });

  it("shows unlimited label when limit is zero", () => {
    render(
      <UsageMeter
        variant="full"
        locale="zh-CN"
        planId="pro"
        marginMultiplier={1.3}
        rolling5h={{
          used: 500,
          limit: 0,
          used_tokens_approx: 500_000,
          limit_tokens_approx: 0,
          percentage: 0,
          reset_at: "2026-06-07T20:00:00Z",
        }}
        rolling7d={{
          used: 2000,
          limit: 0,
          used_tokens_approx: 2_000_000,
          limit_tokens_approx: 0,
          percentage: 0,
          reset_at: "2026-06-10T00:00:00Z",
        }}
        softLimitHit={{ rolling_5h: false, rolling_7d: false }}
        hardLimitHit={{ rolling_5h: false, rolling_7d: false }}
      />,
    );
    expect(screen.getAllByText("无限制").length).toBe(2);
  });
});
