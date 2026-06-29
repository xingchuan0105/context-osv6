import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";
import { isHardCitationGate } from "../../utils/citation-gate";

test.describe("Workspace Chat Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("user can create workspace, chat in general mode, and view history", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // General chat — 消息中包含 runId 以便历史记录识别
    const messageText = `E2E ${runId}: What is the capital of France?`;
    await chat.ask(messageText, "chat");
    await chat.waitForAnswer();

    // 结构性断言（优先）：消息完成标记存在、消息非空
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(0);
    expect(answer).toMatch(/巴黎|Paris/i);

    // Verify history persisted — 提示词库同步即可；首轮 general chat 未必立刻出现在会话列表
    await workspace.waitForHistoryTabVisible();
    await expect(page.getByTestId("query-library-panel").getByText(runId)).toBeVisible();
  });

  test("user can switch to web search mode and get search-grounded answer", async ({ page, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const messageText = `E2E ${runId}: What is the latest Rust release?`;

    await chat.ask(messageText, "search");
    await chat.waitForAnswer(150_000);

    // 结构性断言（优先）：消息完成、消息非空、mode-indicator显示search、citation按钮可见
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
    await expect(page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);

    const citationCount = await chat.citationCount();
    if (isHardCitationGate()) {
      expect(
        citationCount,
        "nightly/staging (E2E_TIER) requires at least one web-search citation",
      ).toBeGreaterThan(0);
    }
    if (citationCount > 0) {
      await chat.expectCitationUiVisible();
    }
  });
});
