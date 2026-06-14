import { test, expect } from "../../fixtures/run-context";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { createNotebookViaAPI, resetAndPrepareTestUser } from "../../utils/api-helpers";
import goldenSet from "../../fixtures/golden_set.json";

test.describe.serial("Chat multi-turn session", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  const entry = goldenSet.entries.find((e) => e.id === "chat-session-01")!;
  if (!entry.turns || entry.turns.length < 2) {
    throw new Error("golden entry chat-session-01 missing turns");
  }

  test("second turn references first turn context", async ({ page, runId }) => {
    test.setTimeout(180_000);

    const notebook = await createNotebookViaAPI(
      page.request,
      `e2e-chat-session ${runId}`,
    );

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
    expect(answer2.length).toBeGreaterThan(20);
    // 上下文连贯：第二轮应延续 antifragility 话题（LLM 不一定每次都点名 Taleb）
    expect(answer2.toLowerCase()).toMatch(/taleb|antifragil|nassim|反脆弱|作者/);
  });
});
