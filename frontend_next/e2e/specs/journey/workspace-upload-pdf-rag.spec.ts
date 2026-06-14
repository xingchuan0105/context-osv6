import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser, waitForDocumentReady } from "../../utils/api-helpers";
import path from "path";

test.describe("PDF Upload + RAG Journey", () => {
  test.describe.configure({ retries: 2 });

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("user can upload PDF, wait for LiteParse ingest, and get RAG-grounded answer", async ({
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

    const fixturePath = path.join(__dirname, "../../fixtures/phase0-mini.pdf");
    await workspace.uploadFile(fixturePath);

    test.slow();
    await workspace.waitForIngestionComplete();

    const documentId = await workspace.getLatestCompletedDocumentId();
    await waitForDocumentReady(request, documentId);

    const messageText = `E2E ${runId}: According to the uploaded PDF, what is LiteParse and how is it used? Cite the document.`;
    await chat.ask(messageText, "rag");
    await chat.waitForAnswer(150_000);

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(20);
    expect(answer.toLowerCase()).not.toContain("could not find relevant evidence");
    expect(answer).toMatch(/LiteParse/i);

    const citationCount = await chat.citationCount();
    expect(citationCount, "PDF RAG journey requires at least one citation").toBeGreaterThan(0);
    await expect(page.locator('[data-testid="workspace-citation"]').first()).toBeVisible();
  });
});
