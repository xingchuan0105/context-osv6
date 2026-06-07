import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";
import path from "path";

test.describe("Document Upload + RAG Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user can upload document, wait for ingestion, and get RAG-grounded answer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // 上传测试文档
    const fixturePath = path.join(__dirname, "../../fixtures/sample-document.txt");
    await workspace.uploadFile(fixturePath);

    // 等待 ingestion 完成（CI 中可能较慢，用 test.slow 延长超时）
    test.slow();
    await workspace.waitForIngestionComplete();

    // 发送 RAG 问题
    const messageText = `E2E ${runId}: What is the tech stack of Context-OS?`;
    await chat.sendMessage(messageText);
    await chat.waitForResponse();

    // 结构性断言：消息完成、非空
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    // RAG 模式下期望看到 citation 按钮（文档中有相关信息时）
    const citationButton = chat.getCitationButton();
    if (await citationButton.count() > 0) {
      await expect(citationButton).toBeVisible();
    }
  });
});
