import { test, expect } from "../../fixtures/run-context";
import { AdminPage } from "../../pom/admin-page";

test.describe("Admin Navigation Smoke", () => {
  test("admin pages load and show expected navigation", async ({ page }) => {
    const admin = new AdminPage(page);
    await admin.goto();
    await admin.expectLoaded();
    await admin.navigateToUsers();
    await admin.expectUserTableVisible();
  });
});
