import { test, expect } from "@playwright/test";

/**
 * Visual regression tests — 生成桌面端和移动端基线快照。
 * 仅覆盖不依赖 LLM 的静态页面，避免 rate limit 导致的快照不稳定。
 * CI 首次运行需加 --update-snapshots；后续运行对比快照 diff。
 */
test.describe("Visual Regression", () => {
  test("V00: login page", async ({ browser }) => {
    // login 页面需要未登录态，否则会被重定向到 dashboard。
    // 认证态存在 localStorage（storageState origins），clearCookies 清不掉，
    // 因此用不带 storageState 的全新 context。
    const context = await browser.newContext({ storageState: { cookies: [], origins: [] } });
    const page = await context.newPage();
    await page.goto("/login");
    await expect(page.locator("#login-email")).toBeVisible();
    await expect(page).toHaveScreenshot("login.png", {
      fullPage: true,
      animations: "disabled",
      // dev overlay（Next.js <nextjs-portal>，含 issue badge）在 dev server 下间歇出现，遮罩防抖动
      mask: [page.locator("nextjs-portal")],
    });
    await context.close();
  });

  test("V01: dashboard page", async ({ page }) => {
    await page.goto("/dashboard");
    await expect(page.getByRole("button", { name: /新建工作区|New workspace/i })).toBeVisible();
    await expect(page).toHaveScreenshot("dashboard.png", {
      fullPage: true,
      animations: "disabled",
      mask: [page.locator("time"), page.locator("nextjs-portal")],
    });
  });
});
