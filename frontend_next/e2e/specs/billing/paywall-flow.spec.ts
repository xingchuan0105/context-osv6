import { test, expect } from "@playwright/test";

import { PaywallPage } from "../../pom/billing-page";

test.describe("Paywall flow", () => {
  test("5h paywall renders 3-tier comparison + 继续 Free", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("5h");
    await paywall.expectVisible();
    await expect(page.getByText(/5h 用量已达上限/)).toBeVisible();
    await expect(page.getByTestId("paywall-continue-free")).toBeVisible();
  });

  test("7d paywall renders 7d-specific title", async ({ page }) => {
    const paywall = new PaywallPage(page);
    await paywall.goto("7d");
    await expect(page.getByText(/7d 用量已达上限/)).toBeVisible();
  });
});
