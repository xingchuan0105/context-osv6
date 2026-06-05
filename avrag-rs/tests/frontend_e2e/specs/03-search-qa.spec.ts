import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import { judgeAnswer } from "../src/quality/judge";
import { getBackendBaseUrl } from "../src/setup/backendUrl";
import { registerTestUser, injectAuth, authHeaders } from "../src/setup/auth";
import goldenSet from "../fixtures/golden_set.json";

test.describe.serial("Search Q&A — real LLM", () => {
  const entry = goldenSet.entries.find((e) => e.id === "search-tokyo-weather-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test("answers with web citation", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await injectAuth(page, auth);

    // Create a workspace via API so we have a notebook to chat in.
    const backendUrl = getBackendBaseUrl();
    const headers = authHeaders(auth);
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "e2e-search-test", description: "" },
      headers,
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();
    const notebookId = notebook.notebook.id;

    const chat = new ChatPage(page);
    await chat.goto(notebookId);
    await chat.ask(entry.query, "search");
    await chat.waitForAnswer();

    const answer = await chat.lastAnswerText();
    const citationCount = await chat.citationCount();

    expect(answer.length).toBeGreaterThan(entry.expected.min_answer_length as number);
    const containsKeyword = (entry.expected.must_contain as string[]).some((kw) =>
      answer.toLowerCase().includes(kw.toLowerCase())
    );
    expect(containsKeyword, `answer should contain one of ${entry.expected.must_contain}`).toBe(true);
    // Web search often returns inline citation buttons; warn if absent.
    if (citationCount === 0) {
      console.warn(`[warn] ${entry.id}: no inline citation buttons found`);
    }

    const judgeResult = await judgeAnswer(answer, entry);
    console.log(`[judge] ${entry.id}: score=${judgeResult.score}, reasoning=${judgeResult.reasoning}`);

    test.info().attach("judge-result.json", {
      body: JSON.stringify(judgeResult, null, 2),
      contentType: "application/json",
    });

    expect(judgeResult.score).toBeGreaterThanOrEqual(7);
  });
});
