import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  createNotebookViaAPI,
  deleteNotebookViaAPI,
  gotoAndWaitForHydration,
  registerTestUser,
  seedBrowserAuth,
} from "./helpers";

const VISUAL_NOTEBOOK_NAME = "visual-regression-notebook";

async function stabilizePage(page: Page): Promise<void> {
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        transition-duration: 0s !important;
        transition-delay: 0s !important;
        animation-duration: 0s !important;
        animation-delay: 0s !important;
        caret-color: transparent !important;
      }
    `,
  });
}

function dynamicMasks(page: Page): Locator[] {
  return [
    page.locator("time"),
    page.locator("[aria-live='polite']"),
    page.locator("[role='status']"),
    page.locator("[class*='toast']"),
    page.locator("[class*='spinner']"),
  ];
}

async function expectVisualSnapshot(page: Page, snapshotName: string): Promise<void> {
  await stabilizePage(page);
  await expect(page).toHaveScreenshot(snapshotName, {
    fullPage: true,
    animations: "disabled",
    caret: "hide",
    mask: dynamicMasks(page),
  });
}

test.describe("Visual UI Regression", () => {
  test("V00: preview dashboard shell", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 810 });
    await gotoAndWaitForHydration(page, "/preview/dashboard");
    await expect(page.getByText("NotebookLM")).toBeVisible();
    await expectVisualSnapshot(page, "preview-dashboard-shell.png");
  });

  test("V01: preview workspace shell", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 810 });
    await gotoAndWaitForHydration(page, "/preview/workspace");
    await expect(page.getByText("Research Project Alpha")).toBeVisible();
    await expectVisualSnapshot(page, "preview-workspace-shell.png");
  });

  test("V02: login page shell", async ({ page }) => {
    await gotoAndWaitForHydration(page, "/login");
    await expect(page.locator("form").first()).toBeVisible();
    await expectVisualSnapshot(page, "login-shell.png");
  });

  test("V03: settings appearance shell", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await seedBrowserAuth(page, request, auth.token);

    await gotoAndWaitForHydration(page, "/settings");
    const appearanceTab = page.getByRole("button", {
      name: /外观与语言|Appearance/,
    });
    await expect(appearanceTab).toBeVisible();
    await appearanceTab.click();
    await expect(page.getByRole("heading", { name: /主题模式|Theme/ })).toBeVisible();
    await expectVisualSnapshot(page, "settings-appearance-shell.png");
  });

  test("V04: workspace shell", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    const notebookId = await createNotebookViaAPI(
      request,
      auth.token,
      VISUAL_NOTEBOOK_NAME,
    );
    await seedBrowserAuth(page, request, auth.token);

    try {
      await gotoAndWaitForHydration(page, `/dashboard/${notebookId}`);
      await expect(page.locator("form textarea").first()).toBeVisible();
      await expectVisualSnapshot(page, "workspace-shell.png");
    } finally {
      await deleteNotebookViaAPI(request, auth.token, notebookId);
    }
  });
});
