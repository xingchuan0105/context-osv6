import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser, waitForDocumentReady } from "../../utils/api-helpers";
import path from "path";

test.describe("Document Upload + RAG Journey", () => {
  test.describe.configure({ retries: 2 });

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("user can upload document, wait for ingestion, and get RAG-grounded answer", async ({
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

    // 问题直接指向 fixture 中的可检索片段（技术架构段落），避免 LLM 误判 evidence_insufficient
    const messageText = `E2E ${runId}: According to the uploaded document, what frontend and backend technologies does Context-OS use? Cite the document.`;
    await chat.ask(messageText, "rag");
    await chat.waitForAnswer(150_000);

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(20);
    expect(answer.toLowerCase()).not.toContain("could not find relevant evidence");
    expect(answer).toMatch(/Next\.js|React|TypeScript|Rust|Milvus/i);

    const citationCount = await chat.citationCount();
    expect(citationCount, "RAG journey requires at least one citation").toBeGreaterThan(0);
    await expect(page.locator('[data-testid="workspace-citation"]').first()).toBeVisible();
  });
});
