import { test, expect } from "../../fixtures/run-context";
import { DashboardPage } from "../../pom/dashboard-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Invite Collaboration", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user A creates workspace and user B accesses via share link", async ({
    browser,
    page,
    runId,
  }) => {
    const dashboard = new DashboardPage(page);

    await page.goto("/dashboard");
    await dashboard.createWorkspace();

    const workspaceId = page.url().match(/\/dashboard\/([^/]+)/)?.[1];
    if (!workspaceId) {
      throw new Error("Failed to extract workspaceId from URL");
    }

    // User A: 进入分享中心，开启公开分享，提取链接
    await page.goto(`/dashboard/${workspaceId}/share`);
    const shareToggle = page.getByRole("switch");
    const isActive = await shareToggle.getAttribute("aria-checked");
    if (isActive !== "true") {
      await shareToggle.click();
    }

    const shareUrlLocator = page.locator("div", {
      hasText: /\/shared\/kb\//,
    });
    await expect(shareUrlLocator).toBeVisible();
    const shareUrl = (await shareUrlLocator.textContent())?.trim();
    if (!shareUrl) {
      throw new Error("Share URL not found in page");
    }

    // User B: 在全新 browser context 中打开分享链接（无需登录）
    const userBContext = await browser.newContext();
    const userBPage = await userBContext.newPage();
    try {
      await userBPage.goto(shareUrl);
      await userBPage.waitForLoadState("networkidle");
      // 验证共享页面加载成功（标题可见，且不是无效链接提示）
      const title = userBPage.locator("h1.app-page-title");
      await expect(title).toBeVisible();
      await expect(title).not.toContainText(/invalid|无效|邀请异常/i);
    } finally {
      await userBContext.close();
    }
  });
});
