import { test, expect } from "../../fixtures/run-context";
import { WorkspacePage } from "../../pom/workspace-page";
import { DashboardPage } from "../../pom/dashboard-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";

test.describe("Workspace CRUD", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("create rename and delete workspace via UI", async ({ page, runId }) => {
    const notebook = new WorkspacePage(page);
    const dashboard = new DashboardPage(page);

    const originalName = `crud-test ${runId}`;
    const renamedName = `crud-test-renamed ${runId}`;

    // Create via UI
    await notebook.createWorkspace(originalName);

    // Verify creation on dashboard
    await page.goto("/dashboard");
    await expect(page.getByText(originalName)).toBeVisible();

    // Rename via UI（须先回到 workspace 页面）
    await dashboard.openWorkspace(originalName);
    await notebook.renameWorkspace(renamedName);

    // Verify rename on dashboard
    await page.goto("/dashboard");
    await expect(page.getByText(renamedName)).toBeVisible();
    await expect(page.getByText(originalName)).not.toBeVisible();

    // Delete via UI（dashboard action menu）
    await notebook.deleteWorkspace();

    // Verify deletion
    await page.goto("/dashboard");
    await expect(page.getByText(renamedName)).not.toBeVisible();
  });
});
