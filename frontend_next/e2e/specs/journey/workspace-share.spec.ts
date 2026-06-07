import { test, expect } from "@playwright/test";
import { DashboardPage } from "../../pom/dashboard-page";
import { SharePage } from "../../pom/share-page";
import { resetTestUserData } from "../../utils/api-helpers";

test.describe("Share & Collaboration Journey", () => {
  test.beforeAll(async ({ request }) => {
    await resetTestUserData(request);
  });

  test("user can enable share and visitor gets read-only access", async ({ page, browser }) => {
    const dashboard = new DashboardPage(page);
    const share = new SharePage(page);

    // 1. 登录用户创建 workspace 并启用分享
    await page.goto("/dashboard");
    await dashboard.createWorkspace();
    const workspaceUrl = page.url();
    const workspaceId = workspaceUrl.split("/dashboard/")[1];

    await share.goto(workspaceId);
    await share.enableShare();
    const shareUrl = await share.copyShareLink();

    expect(shareUrl).toContain("/shared/kb/");

    // 2. 用全新浏览器 context（无登录态）打开分享链接
    const visitorContext = await browser.newContext();
    const visitorPage = await visitorContext.newPage();

    try {
      const resp = await visitorPage.goto(shareUrl);
      // 页面应返回 200（而非 404/403）
      expect(resp?.status()).toBe(200);

      // 等待页面基本渲染完成（h1 出现）
      await expect(visitorPage.locator("h1")).toBeVisible({ timeout: 15_000 });

      // 访客不应看到"添加内容源"按钮（只读验证）
      await expect(visitorPage.getByRole("button", { name: /添加内容源|New source/i })).toHaveCount(0);
    } finally {
      await visitorContext.close();
    }
  });
});
