import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import {
  isPricingRevampEnabled,
  isPricingRevampEnabledClient,
  isPricingRevampEnabledSSR,
} from "../../lib/billing/featureFlag";

describe("featureFlag", () => {
  const original = process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;
  const fetchMock = vi.fn();

  beforeEach(() => {
    process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
    fetchMock.mockReset();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    if (original === undefined) {
      delete process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;
    } else {
      process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = original;
    }
    vi.unstubAllGlobals();
  });

  it("defaults to disabled when env is unset", () => {
    delete process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;
    expect(isPricingRevampEnabledSSR()).toBe(false);
    expect(isPricingRevampEnabledClient()).toBe(false);
  });

  it("enables when NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1", () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "1";
    expect(isPricingRevampEnabledSSR()).toBe(true);
  });

  it("disables when NEXT_PUBLIC_PRICING_REVAMP_ENABLED=0", () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "0";
    expect(isPricingRevampEnabledSSR()).toBe(false);
  });

  it("isPricingRevampEnabled returns false when env is off without fetching", async () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "0";
    await expect(isPricingRevampEnabled()).resolves.toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("isPricingRevampEnabled returns true when probe envelope ok", async () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "1";
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ ok: true, data: {} }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    await expect(isPricingRevampEnabled()).resolves.toBe(true);
  });

  it("isPricingRevampEnabled returns false on feature_disabled envelope", async () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "1";
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: false,
          error: { code: "feature_disabled", message: "not yet available" },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    await expect(isPricingRevampEnabled()).resolves.toBe(false);
  });

  it("isPricingRevampEnabled returns false on HTTP error", async () => {
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "1";
    fetchMock.mockResolvedValueOnce(new Response("", { status: 401 }));
    await expect(isPricingRevampEnabled()).resolves.toBe(false);
  });
});
