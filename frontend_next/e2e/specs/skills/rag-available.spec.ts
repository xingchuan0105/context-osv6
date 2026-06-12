import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData, waitForDocumentReady } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";
import path from "path";

test.describe("RAG Skill Availability", () => {
  test.describe.configure({ retries: 2 });

  const entry = goldenSet.entries.find((e) => e.id === "rag-antifragility-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("RAG mode triggers document retrieval and returns citations", async ({ page, request, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const fixturePath = path.join(__dirname, "../../fixtures/antifragile.txt");
    await workspace.uploadFile(fixturePath);
    await workspace.waitForIngestionComplete();

    const documentId = await workspace.getLatestCompletedDocumentId();
    await waitForDocumentReady(request, documentId);

    const query = entry.query.replace("antifragility", `antifragility ${runId}`);
    await chat.ask(query, "rag");
    await chat.waitForAnswer(150_000);

    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
    await expect(lastMessage.locator("[data-testid='mode-indicator']")).toContainText(
      /rag|文档|检索/i,
    );

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(20);
    expect(answer.toLowerCase()).toMatch(/antifragil|taleb|反脆弱|脆弱/);

    const citationCount = await chat.citationCount();
    expect(citationCount, "RAG golden set requires at least one citation").toBeGreaterThan(0);

    if (process.env.RUN_QUALITY_JUDGE) {
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(answer, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
