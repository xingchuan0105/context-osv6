import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";

test.describe("Query library smoke", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("captures sent prompts and inserts into composer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    const promptText = `E2E ${runId}: Summarize quarterly report`;

    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    await workspace.waitForHistoryTabVisible();

    await chat.sendMessage(promptText);

    const panel = workspace.getQueryLibraryPanel();
    await expect(panel).toBeVisible();
    await expect(panel.getByTestId("query-library-item")).toContainText(promptText);

    const composer = page.getByTestId("workspace-chat-composer");
    await expect(composer).toHaveValue("");

    await chat.waitForResponse(120_000);
    await workspace.clickQueryLibraryItem(promptText);
    await expect(composer).toHaveValue(promptText);
  });
});
