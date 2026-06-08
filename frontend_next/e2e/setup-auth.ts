import { chromium, request } from "@playwright/test";
import { TEST_USER } from "./fixtures/test-user";
import { ensureTestUserAccount } from "./utils/api-helpers";

/**
 * setup-auth 职责：仅负责浏览器登录，生成 storageState。
 *
 * 注意：setup-auth 不验证任何业务功能（如注册表单是否正常）。
 * 业务功能验证由独立的 auth-flow.spec.ts 负责。
 * 两者完全解耦，避免"登录失败"与"业务测试失败"相互混淆。
 */
export default async function setupAuth() {
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000";
  const reqCtx = await request.newContext({ baseURL });
  try {
    await ensureTestUserAccount(reqCtx);
    console.log("[setup-auth] test user ready with admin role");
  } finally {
    await reqCtx.dispose();
  }

  const browser = await chromium.launch();
  const page = await browser.newPage({ baseURL });

  try {
    await page.goto("/login");
    await page.locator("#login-email").fill(TEST_USER.email);
    await page.locator("#login-password").fill(TEST_USER.password);
    await page.getByRole("button", { name: /继续登录/ }).click();

    // 如果登录失败（账号不存在），自动注册
    try {
      await page.waitForURL(/\/dashboard$/, { timeout: 5_000 });
    } catch {
      const stillOnLogin = await page.locator("#login-email").isVisible().catch(() => false);
      if (stillOnLogin) {
        console.log("[setup-auth] login failed, auto-registering test user...");
        await page.goto("/register");
        await page.locator("#register-email").fill(TEST_USER.email);
        await page.locator("#register-password").fill(TEST_USER.password);
        await page.locator("#register-password-confirm").fill(TEST_USER.password);
        await page.locator("#register-name").fill(TEST_USER.fullName);
        await page.getByRole("button", { name: /创建账号|Create account/i }).click();
        // 注册成功后前端自动完成登录并跳转到 dashboard
        await page.waitForURL(/\/dashboard$/, { timeout: 15_000 });
      }
    }

    await page.context().storageState({ path: "playwright/.auth/user.json" });
    console.log("[setup-auth] login succeeded, storageState saved");
  } catch (e) {
    console.error("[setup-auth] login failed:", e);
    throw e;
  } finally {
    await browser.close();
  }
}
