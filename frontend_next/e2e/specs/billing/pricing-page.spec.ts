import { test, expect } from "@playwright/test";

import { PricingPage } from "../../pom/billing-page";

test.describe("Pricing page", () => {
  test("Free user sees 3 tiers with Plus highlighted", async ({ page }) => {
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.expectVisible();
    await expect(page.getByText("推荐")).toBeVisible();
  });

  test("FAQ section is visible and expandable", async ({ page }) => {
    const pricing = new PricingPage(page);
    await pricing.goto();
    await page.getByText("token 用量怎么算？").click();
    await expect(page.getByText(/DeepSeek 公开计费/)).toBeVisible();
  });

  test("clicking 升级 Plus triggers checkout redirect (mocked)", async ({ page }) => {
    await page.route("**/api/v1/billing/checkout-session", (route) =>
      route.fulfill({
        json: { ok: true, data: { checkout_url: "/upgrade/success?mock=1", url: "/upgrade/success?mock=1" } },
      }),
    );
    const pricing = new PricingPage(page);
    await pricing.goto();
    await pricing.clickUpgrade("plus");
    await expect(page).toHaveURL(/\/upgrade\/success/);
  });
});
