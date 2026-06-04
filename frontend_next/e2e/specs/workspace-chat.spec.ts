import { test, expect } from "../fixtures/run-context";
import { DashboardPage } from "../pom/dashboard-page";
import { WorkspacePage } from "../pom/workspace-page";
import { resetTestUserData } from "../utils/api-helpers";

test.describe("Workspace Chat Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user can create workspace, chat in general mode, and view history", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // General chat — 消息中包含 runId 以便历史记录识别
    const messageText = `E2E ${runId}: What is the capital of France?`;
    await workspace.sendMessage(messageText);
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成标记存在、消息非空
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    // Verify history persisted — 断言包含当前 runId 的条目存在
    await workspace.switchToHistoryTab();
    await expect(page.locator(`[data-testid='history-item']:has-text("${runId}")`)).toBeVisible();
  });

  test("user can switch to web search mode and get search-grounded answer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    await workspace.switchToWebSearchMode();

    const messageText = `E2E ${runId}: What is the latest Rust release?`;
    await workspace.sendMessage(messageText);
    await workspace.waitForResponse();

    // 结构性断言（优先）：消息完成、消息非空、mode-indicator显示search、citation按钮可见
    const lastMessage = workspace.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
    await expect(page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);

    // citation 按钮仅在搜索返回 web sources 时出现，属于外部依赖行为，不强求
    const citationButton = workspace.getCitationButton();
    if (await citationButton.count() > 0) {
      await expect(citationButton).toBeVisible();
    }
  });
});
