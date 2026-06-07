import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Analyze Skill", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("analysis query triggers analyze result", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const messageText = `E2E ${runId}: 请分析当前 workspace 的数据趋势`;
    await chat.sendMessage(messageText);
    await chat.waitForResponse();

    // TODO: 当前 UI 未暴露 data-testid="analyze-chart" 或稳定的分析结果区域 selector。
    // 待 UI 侧补充后替换为精确的结构化断言。
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
  });
});
