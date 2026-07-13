import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";

/**
 * Write UI journey (L3).
 *
 * Backend: Playwright `webServer` starts `avrag-api` with project `.env` LLM
 * config (typically **real LLM**). Fast mock coverage is L2 `smoke::write_smoke`
 * — do not treat this journey as a daily mock gate (E2E hardening H3 option C).
 *
 * Wall clock on real LLM is often ~5–6 min (research alone 2–3 min).
 */
test.describe("Workspace Write Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("user can select write mode, send a short topic, and get a non-empty answer", async ({
    page,
    runId,
  }) => {
    // Write = research (Search worker) + skeleton + multi-section draft + refine.
    test.setTimeout(600_000);

    const dashboard = new DashboardPage(page);
    const chat = new ChatPanelPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const topic = `E2E ${runId}: Write a one-paragraph intro about unit testing.`;
    await chat.ask(topic, "write");

    // Mode switch should show write usage hint while composer is in write mode
    await expect(page.getByTestId("workspace-chat-write-usage-hint")).toBeVisible({
      timeout: 10_000,
    });

    // Write emits tokens only at terminal done (no mid-stream answer_start/token).
    // Fail-fast via waitForAnswer error banner; keep wall budget for real LLM research.
    await chat.waitForAnswer(540_000);

    // Assistant-only bubble (H1): avoid matching the trailing user message.
    const lastMessage = chat.getLastAssistantMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    // Surface silent hang: progress card must be gone after waitForAnswer.
    await expect(page.locator('[data-testid="workspace-progress-card"]')).toHaveCount(0);

    await expect(lastMessage.locator("[data-testid='mode-indicator']")).toContainText(
      /write|写作/i,
    );

    const answer = await chat.lastAnswerText();
    // Align with write_smoke substantive floor; still softer than llm_real (80).
    expect(answer.length).toBeGreaterThan(40);
    expect(answer.toLowerCase()).not.toMatch(/stream (failed|error)|internal error|panic/i);
  });
});
