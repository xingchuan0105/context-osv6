import path from "path";
import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser, waitForDocumentReady } from "../../utils/api-helpers";

test.describe("Citation interaction journey", () => {
  test.describe.configure({ retries: 2 });

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("click citation opens chunk preview, thumb-up feedback returns 200 and updates UI", async ({
    page,
    request,
    runId,
  }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const fixturePath = path.join(__dirname, "../../fixtures/sample-document.txt");
    await workspace.uploadFile(fixturePath);

    test.slow();
    await workspace.waitForIngestionComplete();

    const documentId = await workspace.getLatestCompletedDocumentId();
    await waitForDocumentReady(request, documentId);

    const messageText = `E2E ${runId}: According to the uploaded document, what frontend and backend technologies does Context-OS use? Cite the document.`;
    await chat.ask(messageText, "rag");
    await chat.waitForAnswer(150_000);

    const citationCount = await chat.citationCount();
    expect(
      citationCount,
      "citation interaction journey requires at least one citation",
    ).toBeGreaterThan(0);

    // 1. 点击引文 → 弹出"引用片段"预览（原文预览可见）
    const citationChip = page.locator('[data-testid="workspace-citation"]').first();
    await expect(citationChip).toBeVisible();
    await citationChip.click();

    const citationDialog = page.getByRole("dialog", { name: "引用片段" });
    await expect(citationDialog).toBeVisible();
    await expect(citationDialog.getByText(/正在加载引用片段/)).toHaveCount(0, { timeout: 15_000 });

    await page.keyboard.press("Escape");
    await expect(citationDialog).toHaveCount(0);

    // 2. 👍 反馈 → 网络 200 + UI 状态变化（按钮禁用、变为 👍）
    const lastMessage = chat.getLastMessage();
    const thumbUp = lastMessage.getByRole("button", { name: "有用" });
    await expect(thumbUp).toBeVisible();

    const feedbackResponse = page.waitForResponse(
      (resp) => resp.url().includes("/feedback") && resp.request().method() === "POST",
      { timeout: 15_000 },
    );
    await thumbUp.click();
    const resp = await feedbackResponse;
    expect(resp.status()).toBe(200);

    await expect(thumbUp).toBeDisabled();
    await expect(thumbUp).toContainText("👍");
  });
});
