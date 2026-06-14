import { test, expect } from "../../fixtures/run-context";
import { SettingsPage } from "../../pom/settings-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";

test.describe("Billing Usage Meter", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("billing tab shows rolling usage meter and plan display", async ({ page }) => {
    const settings = new SettingsPage(page);

    await settings.gotoBillingTab();
    await settings.expectBillingTabActive();
    await settings.expectBillingSectionLoaded();
    await settings.expectUsageMeterVisible();
    await settings.expectPlanDisplayVisible();
  });
});
