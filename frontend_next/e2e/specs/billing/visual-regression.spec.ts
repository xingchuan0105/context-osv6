import { test, expect } from "@playwright/test";

import { PaywallPage, PricingPage, UsagePage } from "../../pom/billing-page";

const VIEWPORTS = [
  { name: "desktop", width: 1280, height: 800 },
  { name: "mobile", width: 375, height: 667 },
];

for (const vp of VIEWPORTS) {
  test(`Pricing @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const pricing = new PricingPage(page);
    await pricing.goto();
    await expect(page).toHaveScreenshot(`pricing-${vp.name}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.01,
    });
  });

  test(`Usage @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const usage = new UsagePage(page);
    await usage.goto();
    await expect(page).toHaveScreenshot(`usage-${vp.name}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.01,
    });
  });

  test(`Paywall @ ${vp.name}`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height });
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await expect(page).toHaveScreenshot(`paywall-${vp.name}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.01,
    });
  });
}
