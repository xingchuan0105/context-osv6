import { test, expect } from "../../fixtures/run-context";
import { ChatPanelPage } from "../../pom/chat-panel-page";
import { createWorkspaceViaAPI, resetAndPrepareTestUser } from "../../utils/api-helpers";

test.describe("Session history", () => {
  test.beforeAll(async ({ request }) => {
    await resetAndPrepareTestUser(request);
  });

  test("messages survive page refresh", async ({ page, runId }) => {
    const created = await createWorkspaceViaAPI(page.request, `e2e-history-test ${runId}`);
    const workspaceId = created.workspace.id;

    const chat = new ChatPanelPage(page);
    await page.goto(`/dashboard/${workspaceId}`);
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
