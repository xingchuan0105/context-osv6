import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";
import path from "path";

test.describe("Notebook Skill", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("notebook-related query shows document reference after upload", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // Upload a fixture document so the notebook has sources to reference
    const fixturePath = path.join(__dirname, "../../fixtures/antifragile.txt");
    await workspace.uploadFile(fixturePath);
    await workspace.waitForIngestionComplete();

    const messageText = `E2E ${runId}: 这个 notebook 有哪些来源文档？`;
    await chat.sendMessage(messageText);
    await chat.waitForResponse();

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(30);

    // Availability: answer must mention the uploaded document
    const hasDocReference = /antifragile|上传|文件|来源|文档/i.test(answer);
    expect(hasDocReference).toBe(true);

    // TODO: 待 UI 补充 notebook 引用 data-testid 后，添加引用可见性断言
  });
});
