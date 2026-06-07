import { test, expect } from "../../fixtures/run-context";
import { ChatPanelPage } from "../../pom/chat-panel-page";

test.describe("Session history", () => {
  test("messages survive page refresh", async ({ page, runId }) => {
    const notebookRes = await page.request.post("/api/v1/notebooks", {
      data: { name: `e2e-history-test ${runId}`, description: "" },
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    const chat = new ChatPanelPage(page);
    await page.goto(`/dashboard/${notebook.notebook.id}`);
    await page.waitForLoadState("networkidle");

    await chat.ask("What is antifragility?");
    await chat.waitForAnswer();
    const before = await chat.lastAnswerText();
    expect(before.length).toBeGreaterThan(20);

    await page.reload();
    await page.waitForLoadState("networkidle");

    const after = await chat.lastAnswerText();
    expect(after).toContain(before.slice(0, 30)); // approximate match
  });
});
