import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";

/**
 * Soft UI availability for search mode.
 * Hard web-citation gate: L2 search_smoke + L3-thin-llm search_real.
 * Set SKILLS_HARD_CITATION=1 to restore citation hard assert on skills.
 */
test.describe("Search Skill Availability", () => {
  const entry = goldenSet.entries.find((e) => e.id === "search-tokyo-weather-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("Search mode triggers web search and returns citations", async ({ page, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // Switch to search mode and ask
    const query = `${entry.query} ${runId}`;
    await chat.ask(query, "search");
    await chat.waitForResponse(150_000);

    // Availability assertions（结构优先；citation 默认 soft）
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();
    await expect(lastMessage.locator("[data-testid='mode-indicator']")).toContainText(
      /search|联网/i,
    );

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(20);
    expect(answer.toLowerCase()).toMatch(/tokyo|weather|东京|天气|搜索|search/);

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
          "Search skills: no citation chip (soft). Hard gate: search_smoke / search_real",
      });
    }

    // Quality judge (non-blocking report)
    if (process.env.RUN_QUALITY_JUDGE) {
      const answer = await chat.lastAnswerText();
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(answer, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
