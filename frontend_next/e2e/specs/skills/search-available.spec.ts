import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";

test.describe("Search Skill Availability", () => {
  const entry = goldenSet.entries.find((e) => e.id === "search-tokyo-weather-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("Search mode triggers web search and returns citations", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    // Switch to search mode and ask
    const query = `${entry.query} ${runId}`;
    await chat.ask(query, "search");
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
