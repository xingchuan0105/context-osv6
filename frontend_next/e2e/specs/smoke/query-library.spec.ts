import { type Page } from "@playwright/test";
import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { WorkspacePage } from "../../pom/workspace-page";
import { ChatPanelPage } from "../../pom/chat-panel-page";

const QUERY_LIBRARY_STORAGE_KEY = "context-os.query-library.v1";

function workspaceIdFromUrl(url: string) {
  const match = url.match(/\/dashboard\/([^/]+)$/);
  return match?.[1] ?? null;
}

async function seedQueryLibrary(page: Page, workspaceId: string, texts: string[]) {
  const now = Date.now();
  const items = texts.map((text, index) => ({
    id: `e2e-seed-${index}`,
    text,
    createdAt: now - index,
    lastUsedAt: now - index,
    useCount: 1,
  }));

  await page.evaluate(
    ({ key, workspaceId: wsId, seededItems }) => {
      const raw = window.localStorage.getItem(key);
      const persisted = raw
        ? (JSON.parse(raw) as { state: { workspaces: Record<string, unknown> }; version: number })
        : { state: { workspaces: {} }, version: 0 };

      persisted.state.workspaces[wsId] = seededItems;
      window.localStorage.setItem(key, JSON.stringify(persisted));
    },
    { key: QUERY_LIBRARY_STORAGE_KEY, workspaceId, seededItems: items },
  );
}

async function reloadWorkspace(page: Page, workspace: WorkspacePage) {
  const url = page.url();
  await page.goto(url);
  await workspace.waitForHistoryTabVisible();
}

test.describe("Query library smoke", () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test("captures sent prompts and inserts into composer", async ({ page, runId }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);
    const chat = new ChatPanelPage(page);

    const promptText = `E2E ${runId}: Summarize quarterly report`;

    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    await workspace.waitForHistoryTabVisible();

    await chat.sendMessage(promptText);

    const panel = workspace.getQueryLibraryPanel();
    await expect(panel).toBeVisible();
    await expect(panel.getByTestId("query-library-item")).toContainText(promptText);

    const composer = page.getByTestId("workspace-chat-composer");
    await expect(composer).toHaveValue("");

    await chat.waitForAnswer(120_000);
    await workspace.clickQueryLibraryItem(promptText);
    await expect(composer).toHaveValue(promptText);
  });

  test("concatenates prompts when two library items are clicked in sequence", async ({
    page,
    runId,
  }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    const alphaPrompt = `E2E ${runId}: alpha quarterly summary`;
    const betaPrompt = `E2E ${runId}: beta formal rewrite`;

    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    await workspace.waitForHistoryTabVisible();

    const workspaceId = workspaceIdFromUrl(page.url());
    expect(workspaceId).toBeTruthy();

    await seedQueryLibrary(page, workspaceId!, [betaPrompt, alphaPrompt]);
    await reloadWorkspace(page, workspace);

    const panel = workspace.getQueryLibraryPanel();
    await expect(panel.getByText(betaPrompt, { exact: true })).toBeVisible();
    await expect(panel.getByText(alphaPrompt, { exact: true })).toBeVisible();

    const composer = page.getByTestId("workspace-chat-composer");
    await workspace.clickQueryLibraryItem(betaPrompt);
    await workspace.clickQueryLibraryItem(alphaPrompt);

    await expect(composer).toHaveValue(`${betaPrompt}${alphaPrompt}`);
  });

  test("ignores library insert while assistant response is streaming", async ({
    page,
    runId,
  }) => {
    const dashboard = new DashboardPage(page);
    const workspace = new WorkspacePage(page);

    const storedPrompt = `E2E ${runId}: streaming gate stored prompt`;
    const inFlightPrompt = `E2E ${runId}: streaming gate in flight`;

    let chatPostCount = 0;

    await page.route("**/api/v1/chat", async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }

      chatPostCount += 1;
      if (chatPostCount >= 1) {
        await route.fulfill({
          status: 200,
          headers: {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            Connection: "keep-alive",
          },
          body: "event: answer_start\ndata: {\"event\":\"answer_start\"}\n\n",
        });
        return;
      }

      await route.continue();
    });

    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    await workspace.waitForHistoryTabVisible();

    const workspaceId = workspaceIdFromUrl(page.url());
    expect(workspaceId).toBeTruthy();

    await seedQueryLibrary(page, workspaceId!, [storedPrompt]);
    await reloadWorkspace(page, workspace);

    const panel = workspace.getQueryLibraryPanel();
    await expect(panel.getByText(storedPrompt, { exact: true })).toBeVisible();

    const composer = page.getByTestId("workspace-chat-composer");
    await composer.fill(inFlightPrompt);
    await page.getByTestId("workspace-chat-send").click();

    await expect(composer).toHaveValue("");
    await expect(composer).toBeDisabled({ timeout: 10_000 });

    await workspace.clickQueryLibraryItem(storedPrompt);
    await expect(composer).toHaveValue("");

    await page.getByTestId("workspace-chat-stop").click();
  });
});
