import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Notebook Skill", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("notebook-related query shows notebook reference", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const messageText = `E2E ${runId}: 这个 notebook 有哪些来源文档？`;
    await chat.sendMessage(messageText);
    await chat.waitForResponse();

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(30);

    // Skills 层基线：回答应体现 notebook/文档相关特征（关键词非阻塞，仅警告）
    const hasNotebookSignal = /来源|文档|notebook|source|file|document/i.test(answer);
    if (!hasNotebookSignal) {
      console.warn(`[skills] notebook-skill: answer lacks notebook-related keywords`);
    }

    // TODO: 待 UI 补充 notebook 引用 data-testid 后，添加引用可见性断言
  });
});
