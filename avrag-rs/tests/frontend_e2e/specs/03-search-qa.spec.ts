import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import { judgeAnswer } from "../src/quality/judge";
import goldenSet from "../fixtures/golden_set.json";

test.describe.serial("Search Q&A — real LLM", () => {
  const entry = goldenSet.entries.find((e) => e.id === "search-tokyo-weather-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test("open query returns web citation", async ({ page }) => {
    const chat = new ChatPage(page);
    await chat.goto(); // no notebook = open search
    await chat.ask(entry.query);

    const answer = await chat.lastAnswer();
    const citationCount = await chat.citationCount();

    expect(answer.length).toBeGreaterThan(entry.expected.min_answer_length as number);
    expect(citationCount).toBeGreaterThan(0);

    const judgeResult = await judgeAnswer(answer, entry);
    console.log(`[judge] ${entry.id}: score=${judgeResult.score}`);
    test.info().attach("judge-result.json", {
      body: JSON.stringify(judgeResult, null, 2),
      contentType: "application/json",
    });
    expect(judgeResult.score).toBeGreaterThanOrEqual(7);
  });
});
