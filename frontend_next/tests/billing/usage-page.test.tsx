import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

import { resetUsagePageMocks } from "./helpers/usage-page.setup";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createUsagePageMocks());

vi.mock("../../lib/billing/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/billing/api")>();
  return {
    ...actual,
    billingApi: {
      getUsageWindow: vi.fn().mockResolvedValue({
        plan_id: "free",
        margin_multiplier: 2.0,
        rolling_5h: {
          used: 160,
          limit: 200,
          used_tokens_approx: 80_000,
          limit_tokens_approx: 100_000,
          percentage: 80,
          reset_at: "2099-01-01T00:00:00Z",
        },
        rolling_7d: {
          used: 400,
          limit: 800,
          used_tokens_approx: 200_000,
          limit_tokens_approx: 400_000,
          percentage: 50,
          reset_at: "2099-01-01T00:00:00Z",
        },
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
  };
});

vi.mock("../../lib/billing/featureFlag", () => ({
  isPricingRevampEnabledSSR: () => true,
  isPricingRevampEnabled: mocks.isPricingRevampEnabledMock,
  isPricingRevampFeatureDisabledError: () => false,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: mocks.pushMock, replace: mocks.replaceMock }),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({
    initialized: true,
    isAuthenticated: true,
    token: "test-token",
    user: { id: "user-1", email: "user@example.com", full_name: "User" },
    passwordResetEnabled: false,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  }),
}));

import { UsageDashboardClient } from "../../app/(app)/settings/usage/usage-dashboard-client";
import { billingApi } from "../../lib/billing/api";
import { PricingRevampGate } from "../../components/billing/PricingRevampGate";

function renderUsageDashboard() {
  return render(
    <PricingRevampGate redirectTo="/settings">
      <UsageDashboardClient />
    </PricingRevampGate>,
  );
}

describe("UsagePage", () => {
  beforeEach(() => {
    resetUsagePageMocks(mocks);
    vi.mocked(billingApi.getUsageWindow).mockResolvedValue({
      plan_id: "free",
      margin_multiplier: 2.0,
      rolling_5h: {
        used: 160,
        limit: 200,
        used_tokens_approx: 80_000,
        limit_tokens_approx: 100_000,
        percentage: 80,
        reset_at: "2099-01-01T00:00:00Z",
      },
      rolling_7d: {
        used: 400,
        limit: 800,
        used_tokens_approx: 200_000,
        limit_tokens_approx: 400_000,
        percentage: 50,
        reset_at: "2099-01-01T00:00:00Z",
      },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });
  });

  it("renders title + 2 UsageMeter cards + trend chart + forecast", async () => {
    renderUsageDashboard();
    await waitFor(() => {
      expect(screen.getByText(/用量与套餐/)).toBeTruthy();
    });
    expect(screen.getByText(/5 小时窗口/)).toBeTruthy();
    expect(screen.getByText(/7 天窗口/)).toBeTruthy();
    expect(screen.getByText(/近 7 日用量趋势/)).toBeTruthy();
    expect(screen.getByText(/本月无需升级/)).toBeTruthy();
  });

  it("redirects when bucket probe fails", async () => {
    mocks.isPricingRevampEnabledMock.mockResolvedValue(false);
    renderUsageDashboard();
    await waitFor(() => {
      expect(mocks.replaceMock).toHaveBeenCalledWith("/settings");
    });
  });

  it("shows error state when data load fails", async () => {
    vi.mocked(billingApi.getUsageWindow).mockRejectedValueOnce(new Error("network"));
    renderUsageDashboard();
    await waitFor(() => {
      expect(screen.getByText(/用量数据加载失败/)).toBeTruthy();
    });
  });
});
