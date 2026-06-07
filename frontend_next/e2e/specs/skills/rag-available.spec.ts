import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";
import path from "path";

test.describe("RAG Skill Availability", () => {
  const entry = goldenSet.entries.find((e) => e.id === "rag-antifragility-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("RAG mode triggers document retrieval and returns citations", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // Upload fixture document
    const fixturePath = path.join(__dirname, "../../fixtures/antifragile.txt");
    await workspace.uploadFile(fixturePath);
    await workspace.waitForIngestionComplete();

    // Switch to RAG mode and ask
    const query = entry.query.replace("antifragility", `antifragility ${runId}`);
    await chat.ask(query, "rag");
    await chat.waitForAnswer();

    // Availability assertions
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    const citationCount = await chat.citationCount();
    expect(citationCount).toBeGreaterThan(0);

    // Quality judge (non-blocking report)
    if (process.env.RUN_QUALITY_JUDGE) {
      const answer = await chat.lastAnswerText();
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(answer, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
