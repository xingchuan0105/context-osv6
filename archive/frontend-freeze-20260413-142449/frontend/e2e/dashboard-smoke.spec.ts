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

test.describe("Dashboard Smoke", () => {
  let token = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
  });

  test("creates and opens a knowledge base from the dashboard", async ({
    page,
    request,
  }) => {
    await seedLocalAuth(page, token);

    let notebookId = "";
    const name = uniqueName("pw-kb");

    try {
      await gotoStable(page, "/dashboard");
      await waitForAppReady(page);
      await expect(page.getByText("我的知识库")).toBeVisible();

      await page.getByRole("button", { name: /新建知识库/ }).click();
      await page.getByPlaceholder("输入知识库名称").fill(name);
      await page
        .getByPlaceholder("输入知识库描述（可选）")
        .fill("Playwright 创建流程");
      await page.getByRole("button", { name: "确认" }).click();

      await expect(page.getByText("知识库创建成功")).toBeVisible();
      const card = page.getByText(name).first();
      await expect(card).toBeVisible();
      await card.click();

      await expect(page).toHaveURL(/\/dashboard\/[0-9a-f-]+$/);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();
      notebookId = page.url().split("/").pop() || "";
      expect(notebookId).not.toBe("");
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("opens add-source modal and creates a note inside a workspace", async ({
    page,
    request,
  }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-notes"),
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
      await page.getByRole("button", { name: "关闭" }).click();
      await expect(
        page.getByRole("heading", { name: "添加内容源" }),
      ).toHaveCount(0);

      await page.getByRole("button", { name: "笔记" }).first().click();
      await expect(
        page.getByRole("heading", { name: "新建笔记" }),
      ).toBeVisible();

      const noteTitle = uniqueName("pw-note");
      await page.getByPlaceholder("输入笔记标题...").fill(noteTitle);
      await page
        .getByPlaceholder("输入笔记内容...")
        .fill("这是 Playwright 创建的笔记内容。");
      await page.getByRole("button", { name: "保存" }).click();

      await expect(page.getByText("笔记已保存").first()).toBeVisible();
      await expect(page.getByText(noteTitle)).toBeVisible();
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("sends a chat message with mocked SSE response", async ({
    page,
    request,
  }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-chat"),
    );

    try {
      await page.route("**/api/v1/chat?stream=true", async (route) => {
        const sse = [
          "event: start",
          'data: {"session_id":"pw-session","turn_id":"pw-turn-1"}',
          "",
          "event: token",
          'data: {"content":"你好，"}',
          "",
          "event: token",
          'data: {"content":"这是 Playwright 的回放响应。"}',
          "",
          "event: done",
          'data: {"session_id":"pw-session","message_id":1001}',
          "",
        ].join("\n");

        await route.fulfill({
          status: 200,
          contentType: "text/event-stream; charset=utf-8",
          body: sse,
        });
      });

      await seedLocalAuth(page, token);
      await gotoStable(page, `/dashboard/${notebookId}`);
      await waitForAppReady(page);
      await expect(
        page.getByRole("heading", { name: "AI 对话" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "知识库助手" }).click();
      await page.getByRole("button", { name: /通用聊天助手/ }).click();

      const editor = page.locator('[contenteditable="true"]').first();
      await editor.click();
      await editor.fill("请返回一条用于 Playwright 冒烟测试的消息。");
      await page.locator('button[type="submit"]').click();

      await expect(
        page.getByText("你好，这是 Playwright 的回放响应。"),
      ).toBeVisible();
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });
});
