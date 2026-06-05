import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import { getBackendBaseUrl } from "../src/setup/backendUrl";
import { registerTestUser, injectAuth, authHeaders } from "../src/setup/auth";

test.describe.serial("Chat multi-turn session", () => {
  test("second turn references first turn context", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await injectAuth(page, auth);

    const backendUrl = getBackendBaseUrl();
    const headers = authHeaders(auth);
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "e2e-chat-session", description: "" },
      headers,
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    const chat = new ChatPage(page);
    await chat.goto(notebook.notebook.id);

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
