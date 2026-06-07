import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import {
  isPricingRevampEnabledClient,
  isPricingRevampEnabledSSR,
} from "../../lib/billing/featureFlag";

describe("featureFlag", () => {
  const original = process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;

  afterEach(() => {
    if (original === undefined) {
      delete process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED;
    } else {
      process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = original;
    }
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
});
