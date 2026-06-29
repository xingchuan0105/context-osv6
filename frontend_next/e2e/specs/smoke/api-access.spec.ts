import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ApiAccessPage } from "../../pom/api-access-page";
import { runScopedName } from "../../utils/api-helpers";

test.describe("API Access smoke", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("create key shows plaintext once, list shows prefix, revoke updates list", async ({
    page,
    runId,
  }) => {
    const dashboard = new DashboardPage(page);
    const apiAccess = new ApiAccessPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    const workspaceId = page.url().match(/\/dashboard\/([^/]+)$/)?.[1];
    expect(workspaceId).toBeTruthy();

    await apiAccess.goto(workspaceId!);
    await apiAccess.expectApiKeyListVisible();
    await apiAccess.expectEmptyState();

    const keyName = runScopedName("E2E api-access smoke", runId);

    await apiAccess.createApiKey(keyName);
    await apiAccess.expectPlaintextShown();
    await apiAccess.expectKeyItemVisible(keyName);

    await apiAccess.revokeKey(keyName);
    await apiAccess.expectKeyItemGone(keyName);
    await apiAccess.expectEmptyState();
  });
});
