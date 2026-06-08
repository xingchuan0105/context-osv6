import { test, expect } from "../../fixtures/run-context";
import { SettingsPage } from "../../pom/settings-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Billing Settings Smoke", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("settings billing tab shows plan and usage sections", async ({ page }) => {
    const settings = new SettingsPage(page);

    await settings.gotoBillingTab();

    await expect(
      page.getByRole("heading", { name: /^设置$|^Settings$/i }),
    ).toBeVisible();
    await settings.expectBillingTabActive();
    await settings.expectBillingSectionLoaded();

    // Billing UI has no data-testid hooks yet (PR-3 UsageMeter wiring).
    // Assert stable semantic labels for plan status and usage rows.
    await expect(page.getByText(/状态|Status/i).first()).toBeVisible();
    await expect(
      page.getByText(/令牌|Tokens|文档|Documents/i).first(),
    ).toBeVisible();
  });
});
