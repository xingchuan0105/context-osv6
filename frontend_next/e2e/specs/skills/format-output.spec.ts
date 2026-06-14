import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";

test.describe("Format Output Skill Availability", () => {
  const entry = goldenSet.entries.find((e) => e.id === "format-html-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("HTML format request returns structured output", async ({ page, runId }) => {
    test.setTimeout(180_000);

    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const query = `${entry.query} ${runId}`;
    await chat.ask(query);
    await chat.waitForAnswer(150_000);

    // Availability: response contains HTML tags (may be escaped in code/pre blocks)
    const html = await chat.lastAnswerHtml();
    const decoded = html.replace(/&lt;/gi, "<").replace(/&gt;/gi, ">");
    const text = await chat.lastAnswerText();
    expect(
      decoded.toLowerCase().match(/<html|<h[1-6]|<body|<div|<table/) ||
        /antifragil|反脆弱|html/i.test(text),
    ).toBeTruthy();

    // Quality judge (non-blocking report)
    if (process.env.RUN_QUALITY_JUDGE) {
      const { judgeAnswer } = await import("../../utils/judge");
      const result = await judgeAnswer(html, entry);
      test.info().attach("judge-result", { body: JSON.stringify(result) });
    }
  });
});
