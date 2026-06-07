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

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(30);

    // Skills 层基线：回答应体现分析特征（关键词非阻塞，仅警告）
    const hasAnalysisSignal = /趋势|数据|分析|insight|pattern|summary/i.test(answer);
    if (!hasAnalysisSignal) {
      console.warn(`[skills] analyze-skill: answer lacks analysis keywords`);
    }

    // TODO: 待 UI 补充 data-testid="analyze-chart" 后，添加图表可见性断言
  });
});
