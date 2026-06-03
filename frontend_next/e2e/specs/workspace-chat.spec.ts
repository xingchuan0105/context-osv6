import { test, expect } from "../fixtures/run-context";
import { DashboardPage } from "../pom/dashboard-page";
import { WorkspacePage } from "../pom/workspace-page";
import { resetTestUserData, runScopedName } from "../utils/api-helpers";

test.describe("Workspace Chat Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user can create notebook, chat in general mode, and view history", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");

    const notebookName = runScopedName("E2E Chat", runId);
    await dashboard.createNotebook(notebookName);
    await dashboard.openNotebook(notebookName);

    // General chat
    await workspace.sendMessage("What is the capital of France?");
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成标记存在、消息非空
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    // Verify history persisted — 断言包含当前runId前缀的条目存在
    await workspace.switchToHistoryTab();
    await expect(page.locator(`[data-testid='history-item']:has-text("${runId}")`)).toBeVisible();
  });

  test("user can switch to web search mode and get search-grounded answer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");

    const notebookName = runScopedName("E2E WebSearch", runId);
    await dashboard.createNotebook(notebookName);
    await dashboard.openNotebook(notebookName);

    await workspace.switchToWebSearchMode();

    // 结构性断言（优先）：mode indicator显示正确模式
    await expect(page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);

    await workspace.sendMessage("What is the latest Rust release?");
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成、消息非空、citation按钮可见
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    const citationButton = workspace.getCitationButton();
    await expect(citationButton).toBeVisible();
  });
});
