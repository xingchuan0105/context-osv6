import { test, expect } from "../../fixtures/run-context";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import goldenSet from "../../fixtures/golden_set.json";

test.describe.serial("Chat multi-turn session", () => {
  const entry = goldenSet.entries.find((e) => e.id === "chat-session-01")!;
  if (!entry.turns || entry.turns.length < 2) {
    throw new Error("golden entry chat-session-01 missing turns");
  }

  test("second turn references first turn context", async ({ page, runId }) => {
    // Create notebook via API using page.request (carries auth cookies from storageState)
    const notebookRes = await page.request.post("/api/v1/notebooks", {
      data: { name: `e2e-chat-session ${runId}`, description: "" },
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    const chat = new ChatPanelPage(page);
    await page.goto(`/dashboard/${notebook.notebook.id}`);
    await page.waitForLoadState("networkidle");

    // Turn 1: ask from golden_set
    await chat.ask(entry.turns[0]);
    await chat.waitForAnswer();
    const answer1 = await chat.lastAnswerText();
    expect(answer1.length).toBeGreaterThan(20);

    // Turn 2: context-dependent follow-up from golden_set
    await chat.ask(entry.turns[1]);
    await chat.waitForAnswer();
    const answer2 = await chat.lastAnswerText();
    expect(answer2.toLowerCase()).toContain("taleb");
  });
});
