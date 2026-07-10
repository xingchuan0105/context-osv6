import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { resetAndPrepareTestUser } from "../../utils/api-helpers";

test.describe("Workspace Write Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("user can select write mode, send a short topic, and get a non-empty answer", async ({
    page,
    runId,
  }) => {
    // Write = research (Search worker) + skeleton + multi-section draft + refine.
    // Mock is short; real LLM on this stack is ~5 min (research alone ~2–3 min).
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
    await chat.waitForAnswer(540_000);

    // Structural assertions: completed assistant message, non-empty body, write mode tag
    const lastMessage = chat.getLastMessage();
    await expect(lastMessage).toBeVisible();
    await expect(lastMessage).not.toBeEmpty();

    await expect(lastMessage.locator("[data-testid='mode-indicator']")).toContainText(
      /write|写作/i,
    );

    const answer = await chat.lastAnswerText();
    expect(answer.length).toBeGreaterThan(0);
  });
});
