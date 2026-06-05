import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import goldenSet from "../fixtures/golden_set.json";

test.describe.serial("Format output", () => {
  const entry = goldenSet.entries.find((e) => e.id === "format-html-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test("HTML format request returns structured output", async ({ page }) => {
    const chat = new ChatPage(page);
    await chat.goto();
    await chat.ask(entry.query);

    const answer = await chat.lastAnswer();
    expect(answer.toLowerCase()).toContain("<html>");
    expect(answer.toLowerCase()).toContain("antifragil");
  });
});
