import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Analyze Skill", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("analysis query triggers analyze result", async ({ page, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const messageText = `E2E ${runId}: 请分析当前 workspace 的数据趋势`;
    await chat.sendMessage(messageText);
    await chat.waitForAnswer(150_000);

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(30);

    // Availability: answer must show analysis characteristics
    const hasAnalysisSignal = /趋势|数据|分析|insight|pattern|summary/i.test(answer);
    expect(hasAnalysisSignal).toBe(true);

    // TODO: 待 UI 补充 data-testid="analyze-chart" 后，添加图表可见性断言
  });
});
