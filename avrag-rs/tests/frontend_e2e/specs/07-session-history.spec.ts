import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import { getBackendBaseUrl } from "../src/setup/backendUrl";
import { registerTestUser, injectAuth, authHeaders } from "../src/setup/auth";

test.describe("Session history", () => {
  test("messages survive page refresh", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await injectAuth(page, auth);

    const backendUrl = getBackendBaseUrl();
    const headers = authHeaders(auth);
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "e2e-history-test", description: "" },
      headers,
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    const chat = new ChatPage(page);
    await chat.goto(notebook.notebook.id);
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
