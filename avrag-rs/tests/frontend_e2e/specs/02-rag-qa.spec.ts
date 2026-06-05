import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";
import { judgeAnswer } from "../src/quality/judge";
import goldenSet from "../fixtures/golden_set.json";

test.describe.serial("RAG Q&A — real LLM", () => {
  const entry = goldenSet.entries.find((e) => e.id === "rag-antifragility-01")!;
  if (!entry.query) throw new Error("golden entry missing query");

  test("uploads document and answers with citation", async ({ page, request }) => {
    // 1. Upload document via backend API (faster than UI click-through)
    const backendUrl = process.env.BACKEND_BASE_URL || "http://127.0.0.1:8080";
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "e2e-rag-test", description: "" },
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();
    const notebookId = notebook.notebook.id;

    // 2. Upload fixture file
    const fixturePath = "../fixtures/documents/antifragile.txt";
    const uploadInit = await request.post(`${backendUrl}/api/v1/notebooks/${notebookId}/documents`, {
      data: { filename: "antifragile.txt", file_size: 1728, mime_type: "text/plain" },
    });
    expect(uploadInit.status()).toBe(200);
    const { document_id: docId } = await uploadInit.json();

    const fileContent = await import("fs").then((fs) => fs.promises.readFile(fixturePath, "utf-8"));
    const putRes = await request.put(`${backendUrl}/dev-upload/${docId}`, {
      data: fileContent,
    });
    expect(putRes.status()).toBe(200);

    // 3. Poll ingestion status (up to 180s)
    let status = "queued";
    const deadline = Date.now() + 180_000;
    while (status !== "completed" && Date.now() < deadline) {
      const s = await request.get(`${backendUrl}/api/v1/documents/${docId}/status`);
      const body = await s.json();
      status = body.status;
      if (status === "failed") throw new Error("Ingestion failed");
      await new Promise((r) => setTimeout(r, 500));
    }
    expect(status).toBe("completed");

    // 4. Open chat page and ask question
    const chat = new ChatPage(page);
    await chat.goto(notebookId);
    await chat.ask(entry.query);

    // 5. Collect answer and citations
    const answer = await chat.lastAnswer();
    const citationCount = await chat.citationCount();

    // 6. Hard assertions (product rules)
    expect(answer.length).toBeGreaterThan(entry.expected.min_answer_length as number);
    const containsKeyword = (entry.expected.must_contain as string[]).some((kw) =>
      answer.toLowerCase().includes(kw.toLowerCase())
    );
    expect(containsKeyword, `answer should contain one of ${entry.expected.must_contain}`).toBe(true);
    expect(citationCount).toBeGreaterThan(0);

    // 7. LLM judge (quality assessment)
    const judgeResult = await judgeAnswer(answer, entry);
    console.log(`[judge] ${entry.id}: score=${judgeResult.score}, reasoning=${judgeResult.reasoning}`);

    // Attach to Playwright report
    test.info().attach("judge-result.json", {
      body: JSON.stringify(judgeResult, null, 2),
      contentType: "application/json",
    });

    // Score gate: must be >= 7/10
    expect(judgeResult.score).toBeGreaterThanOrEqual(7);
  });
});
