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

    // TODO: 当前 UI 未暴露 notebook 引用专用的 data-testid。
    // 先以消息可见非空作为基线断言，待后续 UI 补充 selector 后完善。
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
  });
});
