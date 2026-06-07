import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetTestUserData } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";

test.describe("Format Output Skill Availability", () => {
  const entry = goldenSet.entries.find((e) => e.id === "format-html-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("HTML format request returns structured output", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const query = `${entry.query} ${runId}`;
    await chat.ask(query);
    await chat.waitForAnswer();

    // Availability: response contains HTML tags
    const html = await chat.lastAnswerHtml();
    expect(html.toLowerCase()).toContain("<html");

    // Quality judge (non-blocking report)
    if (process.env.RUN_QUALITY_JUDGE) {
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(html, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
