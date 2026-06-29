import { test, expect } from "@playwright/test";

import { UsagePage } from "../../pom/billing-page";

test.describe("Usage dashboard", () => {
  test("Free user sees 2 buckets + trend chart + forecast", async ({ page }) => {
    const usage = new UsagePage(page);
    await usage.goto();
    await usage.expectVisible();
    await expect(page.getByText(/近 7 日用量趋势/)).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText(/智能建议|按当前用量|本月无需升级/)).toBeVisible({ timeout: 30_000 });
  });

  test("shows warning text when 5h soft limit hit (via mocked API)", async ({ page }) => {
    await page.route("**/api/v1/billing/usage/window", (route) =>
      route.fulfill({
        json: {
          ok: true,
          data: {
            plan_id: "free",
            rolling_5h: {
              used: 85000,
              limit: 100000,
              percentage: 85,
              reset_at: "2099-01-01T00:00:00Z",
            },
            rolling_7d: {
              used: 200000,
              limit: 400000,
              percentage: 50,
              reset_at: "2099-01-01T00:00:00Z",
            },
            soft_limit_hit: { rolling_5h: true, rolling_7d: false },
            hard_limit_hit: { rolling_5h: false, rolling_7d: false },
          },
        },
      }),
    );
    const usage = new UsagePage(page);
    await usage.goto();
    await expect(page.getByText(/已超过软上限/)).toBeVisible({ timeout: 30_000 });
  });
});
