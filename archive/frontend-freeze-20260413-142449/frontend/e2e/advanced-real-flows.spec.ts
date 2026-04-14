import { expect, test } from "@playwright/test";

import {
  createNotebookViaAPI,
  deleteNotebookViaAPI,
  gotoStable,
  registerTestUser,
  seedLocalAuth,
  uniqueName,
  waitForAppReady,
} from "./helpers";

const realUploadFile =
  process.env.PLAYWRIGHT_UPLOAD_FILE || "/mnt/d/test/akerlof.pdf";

test.describe("Advanced Real Flows", () => {
  let token = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
  });

  test("uploads a real PDF through the workspace UI", async ({
    page,
    request,
  }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-upload-real"),
    );

    try {
      await seedLocalAuth(page, token);
      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "添加内容源" }).click();
      await expect(
        page.getByRole("heading", { name: "添加内容源" }),
      ).toBeVisible();

      await page.locator('input[type="file"]').setInputFiles(realUploadFile);

      await expect(
        page.getByRole("heading", { name: "添加内容源" }),
      ).toHaveCount(0, { timeout: 20_000 });
      await expect(page.getByText("akerlof.pdf")).toBeVisible({
        timeout: 20_000,
      });
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("creates a real share link and opens the shared page", async ({
    page,
    request,
    context,
  }) => {
    const notebookTitle = uniqueName("pw-share-real");
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      notebookTitle,
    );

    try {
      await seedLocalAuth(page, token);
      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "分享" }).click();
      await expect(
        page.getByRole("heading", { name: "分享知识库" }),
      ).toBeVisible();

      const shareInput = page.locator("input[readonly]").first();
      await expect(shareInput).not.toHaveValue("", { timeout: 15_000 });
      const shareLink = await shareInput.inputValue();
      expect(shareLink).toContain("/shared/kb/");

      const sharedPage = await context.newPage();
      await seedLocalAuth(sharedPage, token);
      await gotoStable(sharedPage, shareLink);
      await expect(
        sharedPage.getByRole("heading", { name: notebookTitle }),
      ).toBeVisible();
      await expect(sharedPage.getByText("权限:")).toBeVisible();
      await sharedPage.close();
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("sends a real general chat request through the UI", async ({
    page,
    request,
  }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-chat-real"),
    );

    try {
      await seedLocalAuth(page, token);
      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "知识库助手" }).click();
      await page.getByRole("button", { name: /通用聊天助手/ }).click();

      const marker = uniqueName("chat");
      const prompt = `请回复一句短话，包含标记 ${marker}`;
      const editor = page.locator('[contenteditable="true"]').first();
      await editor.click();
      await editor.fill(prompt);
      await page.locator('button[type="submit"]').click();

      const bubbles = page.locator("div.whitespace-pre-wrap");
      await expect(bubbles).toHaveCount(2, { timeout: 30_000 });
      await expect(bubbles.nth(1)).toContainText(marker, { timeout: 30_000 });

      await page.reload({ waitUntil: "domcontentloaded" });
      await waitForAppReady(page);
      const restoredBubbles = page.locator("div.whitespace-pre-wrap");
      await expect(restoredBubbles.nth(0)).toContainText(prompt, {
        timeout: 30_000,
      });
      await expect(restoredBubbles.nth(1)).toContainText(marker, {
        timeout: 30_000,
      });
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("sends a real search chat request through the UI", async ({
    page,
    request,
  }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-search-real"),
    );

    try {
      await seedLocalAuth(page, token);
      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "知识库助手" }).click();
      await page.getByRole("button", { name: /网络搜索助手/ }).click();

      const prompt = "杭州天气";
      const editor = page.locator('[contenteditable="true"]').first();
      await editor.click();
      await editor.fill(prompt);
      await page.locator('button[type="submit"]').click();

      const bubbles = page.locator("div.whitespace-pre-wrap");
      await expect(bubbles).toHaveCount(2, { timeout: 90_000 });
      await expect(bubbles.nth(1)).not.toHaveText("", { timeout: 90_000 });
      await expect(page.getByText("🔍 Search")).toBeVisible();
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });
});

test.describe("Mobile Viewport", () => {
  test.use({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });

  let token = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
  });

  test("opens dashboard menu and workspace sidebar on mobile", async ({
    page,
    request,
  }) => {
    const notebookTitle = uniqueName("pw-mobile");
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      notebookTitle,
    );

    try {
      await seedLocalAuth(page, token);
      await gotoStable(page, "/dashboard");
      await waitForAppReady(page);
      await expect(
        page.getByRole("button", { name: "打开菜单" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "打开菜单" }).click();
      await expect(page.getByText("菜单")).toBeVisible();
      await page.getByRole("button", { name: "工作区列表" }).click();

      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("button", { name: "打开侧边栏" }),
      ).toBeVisible();
      await page.getByRole("button", { name: "打开侧边栏" }).click();
      await expect(page.getByRole("heading", { name: "内容源" })).toBeVisible();
      await expect(
        page.getByRole("button", { name: "添加内容源" }),
      ).toBeVisible();
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });
});
