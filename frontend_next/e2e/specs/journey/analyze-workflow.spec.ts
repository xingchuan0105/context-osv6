import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { AnalyzePage } from "../../pom/analyze-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";
import path from "path";

test.describe("Analyze Workflow", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("upload document and view analyze insights", async ({ page }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const analyze = new AnalyzePage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const workspaceId = page.url().match(/\/dashboard\/([^/]+)/)?.[1];
    if (!workspaceId) {
      throw new Error("Failed to extract workspaceId from URL after creation");
    }

    const fixturePath = path.join(__dirname, "../../fixtures/sample-document.txt");
    await workspace.uploadFile(fixturePath);
    await workspace.waitForIngestionComplete();

    await analyze.goto(workspaceId);
    await analyze.expectInsightVisible();
  });
});
