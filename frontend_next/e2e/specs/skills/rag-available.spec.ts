import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser, waitForDocumentReady } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";
import path from "path";

/**
 * Soft UI availability for RAG mode (mode indicator + answer).
 *
 * Hard citation / upload→RAG product gate lives on:
 *   - L2: product_e2e smoke rag_smoke
 *   - L3-thin-llm: llm_real rag_real (standard doc)
 *   - L3-journey: workspace-upload-rag.spec.ts
 * Skills project is nightly/display; do not double hard-gate citations unless
 * SKILLS_HARD_CITATION=1.
 */
test.describe("RAG Skill Availability", () => {
  test.describe.configure({ retries: 2 });

  const entry = goldenSet.entries.find((e) => e.id === "rag-antifragility-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("RAG mode triggers document retrieval and returns citations", async ({ page, request, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // Same standard product fixture as journey + llm_real (antifragile.txt).
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
    const hardCitation =
      process.env.SKILLS_HARD_CITATION === "1" ||
      process.env.SKILLS_HARD_CITATION === "true";
    if (hardCitation) {
      expect(citationCount, "SKILLS_HARD_CITATION=1 requires ≥1 citation").toBeGreaterThan(0);
    } else if (citationCount === 0) {
      test.info().annotations.push({
        type: "soft",
        description:
          "RAG skills: no citation chip (soft). Hard UI RAG gate: journey workspace-upload-rag / llm_real rag_real",
      });
    }

    if (process.env.RUN_QUALITY_JUDGE) {
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(answer, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
