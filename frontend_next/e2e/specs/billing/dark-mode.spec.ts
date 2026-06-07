import { test, expect } from "@playwright/test";

import { PaywallPage, PricingPage, UsagePage } from "../../pom/billing-page";

test.describe("Dark mode", () => {
  test.use({ colorScheme: "dark" });

  test("Pricing page renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.expectVisible();
    await page.screenshot({ path: "test-results/pricing-dark.png", fullPage: true });
  });

  test("Usage dashboard renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const usage = new UsagePage(page);
    await usage.goto();
    await usage.expectVisible();
    await page.screenshot({ path: "test-results/usage-dark.png", fullPage: true });
  });

  test("Paywall renders correctly in dark mode", async ({ page }) => {
    await page.evaluate(() => document.documentElement.setAttribute("data-theme", "dark"));
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await paywall.expectVisible();
    await page.screenshot({ path: "test-results/paywall-dark.png", fullPage: true });
  });
});
