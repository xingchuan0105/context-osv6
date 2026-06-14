import { test, expect } from "../../fixtures/run-context";
import { NotebookPage } from "../../pom/notebook-page";
import { DashboardPage } from "../../pom/dashboard-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";

test.describe("Notebook CRUD", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("create rename and delete notebook via UI", async ({ page, runId }) => {
    const notebook = new NotebookPage(page);
    const dashboard = new DashboardPage(page);

    const originalName = `crud-test ${runId}`;
    const renamedName = `crud-test-renamed ${runId}`;

    // Create via UI
    await notebook.createNotebook(originalName);

    // Verify creation on dashboard
    await page.goto("/dashboard");
    await expect(page.getByText(originalName)).toBeVisible();

    // Rename via UI（须先回到 workspace 页面）
    await dashboard.openWorkspace(originalName);
    await notebook.renameNotebook(renamedName);

    // Verify rename on dashboard
    await page.goto("/dashboard");
    await expect(page.getByText(renamedName)).toBeVisible();
    await expect(page.getByText(originalName)).not.toBeVisible();

    // Delete via UI（dashboard action menu）
    await notebook.deleteNotebook();

    // Verify deletion
    await page.goto("/dashboard");
    await expect(page.getByText(renamedName)).not.toBeVisible();
  });
});
