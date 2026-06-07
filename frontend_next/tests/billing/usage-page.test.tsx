import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";

vi.mock("../../lib/billing/api", () => ({
  billingApi: {
    getUsageWindow: vi.fn().mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 80000, limit: 100000, percentage: 80, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    }),
    getUsageHistory: vi.fn().mockResolvedValue({
      daily: [
        { date: "2026-06-01", tokens: 50000 },
        { date: "2026-06-02", tokens: 75000 },
      ],
    }),
    getUsageForecast: vi.fn().mockResolvedValue({
      current_plan: "free",
      avg_30d_tokens: 8000,
      projected_30d_tokens: 240000,
      current_limit_7d: 400000,
      upgrade_recommended: false,
      suggestion_zh: "按当前用量，本月无需升级",
      suggestion_en: "Based on current usage, no upgrade needed",
    }),
  },
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock("next-intl", () => ({
  useLocale: () => "zh-CN",
}));

import { UsageDashboardClient } from "../../app/(app)/settings/usage/usage-dashboard-client";

describe("UsagePage", () => {
  it("renders title + 2 UsageMeter cards + trend chart + forecast", async () => {
    render(<UsageDashboardClient />);
    await waitFor(() => {
      expect(screen.getByText(/用量与套餐/)).toBeTruthy();
    });
    expect(screen.getByText(/5 小时窗口/)).toBeTruthy();
    expect(screen.getByText(/7 天窗口/)).toBeTruthy();
    expect(screen.getByText(/近 7 日用量趋势/)).toBeTruthy();
    expect(screen.getByText(/本月无需升级/)).toBeTruthy();
  });
});
