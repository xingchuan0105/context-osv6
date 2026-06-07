import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { billingApi } from "../../lib/billing/api";

const fetchMock = vi.fn();

beforeEach(() => {
  process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  delete process.env.NEXT_PUBLIC_API_BASE_URL;
  vi.unstubAllGlobals();
});

describe("billingApi.getPlans", () => {
  it("returns parsed BillingPlansResponse", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            plans: [
              {
                plan_id: "pro",
                name: "Pro",
                description: "Pro tier",
                price_label: "$29.00",
                price_label_cny: "¥199.00",
                price_label_usd: "$29.00",
                interval: "month",
                checkout_available: true,
                current: true,
                quotas: [
                  { metric_type: "embedding_tokens", soft_limit: 1_000_000, hard_limit: null },
                ],
              },
            ],
            current_plan_id: "pro",
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await billingApi.getPlans();

    expect(result.plans[0].plan_id).toBe("pro");
    expect(result.plans[0].price_label_cny).toBe("¥199.00");
    expect(result.current_plan_id).toBe("pro");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/billing/plans",
      expect.objectContaining({ method: "GET", credentials: "include" }),
    );
  });
});

describe("billingApi.getUsageWindow", () => {
  it("returns parsed UsageWindowResponse", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            plan_id: "free",
            rolling_5h: { used: 80_000, limit: 100_000, percentage: 80, reset_at: "2026-06-07T20:00:00Z" },
            rolling_7d: { used: 200_000, limit: 400_000, percentage: 50, reset_at: "2026-06-10T00:00:00Z" },
            soft_limit_hit: { rolling_5h: true, rolling_7d: false },
            hard_limit_hit: { rolling_5h: false, rolling_7d: false },
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await billingApi.getUsageWindow();

    expect(result.plan_id).toBe("free");
    expect(result.rolling_5h.percentage).toBe(80);
    expect(result.soft_limit_hit.rolling_5h).toBe(true);
  });

  it("throws on non-ok response", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ ok: false, message: "boom" }), { status: 500 }),
    );
    await expect(billingApi.getUsageWindow()).rejects.toThrow();
  });

  it("throws when envelope is not ok", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({ ok: false, error: { message: "no access" } }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    await expect(billingApi.getUsageWindow()).rejects.toThrow("no access");
  });
});

describe("billingApi.getUsageHistory", () => {
  it("uses the days query parameter", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: { daily: [{ date: "2026-06-07", tokens: 1234 }] },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await billingApi.getUsageHistory(14);
    expect(result.daily[0].tokens).toBe(1234);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/billing/usage/history?days=14",
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("defaults to 7 days", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({ ok: true, data: { daily: [] } }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    await billingApi.getUsageHistory();
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/billing/usage/history?days=7",
      expect.anything(),
    );
  });
});

describe("billingApi.getUsageForecast", () => {
  it("returns parsed forecast", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            current_plan: "free",
            avg_30d_tokens: 50_000,
            projected_30d_tokens: 1_500_000,
            current_limit_7d: 400_000,
            upgrade_recommended: true,
            suggestion_zh: "建议升级",
            suggestion_en: "Consider upgrading",
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await billingApi.getUsageForecast();
    expect(result.upgrade_recommended).toBe(true);
    expect(result.suggestion_en).toBe("Consider upgrading");
  });
});
