import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import goldenSet from "../fixtures/golden_set.json";
import { getBackendBaseUrl } from "../src/setup/backendUrl";
import { registerTestUser, injectAuth, authHeaders } from "../src/setup/auth";

test.describe.serial("Format output", () => {
  const entry = goldenSet.entries.find((e) => e.id === "format-html-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test("HTML format request returns structured output", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await injectAuth(page, auth);

    const backendUrl = getBackendBaseUrl();
    const headers = authHeaders(auth);
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "e2e-format-test", description: "" },
      headers,
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    const chat = new ChatPage(page);
    await chat.goto(notebook.notebook.id);
    await chat.ask(entry.query);
    await chat.waitForAnswer();

    const raw = await chat.lastAnswerRawText();
    expect(raw.toLowerCase()).toContain("<html");
    const text = await chat.lastAnswerText();
    expect(text.toLowerCase()).toContain("antifragil");
  });
});
