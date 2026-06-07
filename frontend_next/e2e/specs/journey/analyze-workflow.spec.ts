import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { AnalyzePage } from "../../pom/analyze-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Analyze Workflow", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("upload document and view analyze insights", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const analyze = new AnalyzePage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const workspaceId = page.url().match(/\/dashboard\/([^/]+)/)?.[1];
    if (!workspaceId) {
      throw new Error("Failed to extract workspaceId from URL after creation");
    }

    await workspace.uploadFile("e2e/fixtures/sample-document.txt");
    await workspace.waitForIngestionComplete();

    await analyze.goto(workspaceId);
    await analyze.expectInsightVisible();
  });
});
