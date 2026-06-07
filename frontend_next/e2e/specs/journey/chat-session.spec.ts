import { test, expect } from "../../fixtures/run-context";
import { ChatPanelPage } from "../../pom/chat-panel-page";

test.describe.serial("Chat multi-turn session", () => {
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

    await chat.ask("What is antifragility?");
    await chat.waitForAnswer();
    const answer1 = await chat.lastAnswerText();
    expect(answer1.length).toBeGreaterThan(20);

    await chat.ask("Who wrote the book about it?");
    await chat.waitForAnswer();
    const answer2 = await chat.lastAnswerText();
    expect(answer2.toLowerCase()).toContain("taleb");
  });
});
