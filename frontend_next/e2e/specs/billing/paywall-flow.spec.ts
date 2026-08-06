import { test, expect } from "@playwright/test";

import { PaywallPage } from "../../pom/billing-page";

test.describe("Paywall flow", () => {
  test("5h protective paywall renders upgrade options", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await paywall.expectVisible();
    // ADR-0010: titles are protective-limit copy, not "5h 用量已达上限".
    await expect(page.getByText(/平台保护限速|protective limit/i)).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByTestId("paywall-continue-free")).toBeVisible({ timeout: 30_000 });
  });

  test("7d protective paywall renders same protective framing", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("7d");
    await expect(page.getByText(/平台保护限速|protective limit/i)).toBeVisible({
      timeout: 30_000,
    });
  });
});
