import { test, expect } from "@playwright/test";
import { LoginPage } from "../pom/login-page";

/**
 * auth-flow.spec.ts 职责：验证注册/登录/登出的业务功能。
 *
 * 注意：setup-env + setup-auth 已独立完成环境准备（reset + login + storageState）。
 * 本spec不依赖 setup 的执行结果，也不被任何 project 依赖。
 * 使用空 storageState 确保每次测试都是"未登录"的干净状态。
 */
test.use({ storageState: { cookies: [], origins: [] } });

test.describe("Auth Flow", () => {
  test("user can register and login via UI", async ({ page }) => {
    const email = `e2e-${Date.now()}@test.local`;

    // Register
    await page.goto("/register");
    await page.locator("#register-email").fill(email);
    await page.locator("#register-password").fill("E2eTest123!");
    await page.locator("#register-password-confirm").fill("E2eTest123!");
    await page.locator("#register-name").fill("E2E User");
    await page.getByRole("button", { name: /创建账号|Create account/i }).click();
    // 注册成功后前端自动完成登录并跳转到 dashboard
    await page.waitForURL(/\/dashboard$/, { timeout: 15_000 });
  });

  test("login page does not submit before hydration", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false, locale: "zh-CN" });
    const page = await context.newPage();

    try {
      await page.goto("/login");
      await page.locator("#login-email").fill("prehydrate@example.com");
      await page.locator("#login-password").fill("E2eTest123!");
      await page.locator("#login-password").press("Enter");
      await expect(page).toHaveURL(/\/login$/);
    } finally {
      await context.close();
    }
  });
});
