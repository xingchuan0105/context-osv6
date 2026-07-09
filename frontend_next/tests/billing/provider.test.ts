import { describe, expect, it } from "vitest";

import { billingProviderForLocale, planPriceLabel } from "../../lib/billing/provider";

describe("billing provider helpers", () => {
  it("routes zh-CN users to Alipay", () => {
    expect(billingProviderForLocale("zh-CN")).toBe("alipay");
  });

  it("routes English users to Creem", () => {
    expect(billingProviderForLocale("en")).toBe("creem");
  });

  it("picks locale-specific price labels from API payloads", () => {
    const plan = {
      price_label: "¥19.00 / 月 · $3.19 / 月",
      price_label_cny: "¥19.00 / 月",
      price_label_usd: "$3.19 / 月",
    };

    expect(planPriceLabel(plan, "zh-CN")).toBe("¥19.00 / 月");
    expect(planPriceLabel(plan, "en")).toBe("$3.19 / 月");
  });
});
