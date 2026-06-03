import { chromium } from "@playwright/test";
import { TEST_USER } from "./fixtures/test-user";

/**
 * setup-auth 职责：仅负责浏览器登录，生成 storageState。
 *
 * 注意：setup-auth 不验证任何业务功能（如注册表单是否正常）。
 * 业务功能验证由独立的 auth-flow.spec.ts 负责。
 * 两者完全解耦，避免"登录失败"与"业务测试失败"相互混淆。
 */
export default async function setupAuth() {
  const browser = await chromium.launch();
  const page = await browser.newPage({
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
  });

  try {
    await page.goto("/login");
    await page.locator("#login-email").fill(TEST_USER.email);
    await page.locator("#login-password").fill(TEST_USER.password);
    await page.getByRole("button", { name: /继续登录/ }).click();
    await page.waitForURL(/\/dashboard$/, { timeout: 15_000 });

    await page.context().storageState({ path: "playwright/.auth/user.json" });
    console.log("[setup-auth] login succeeded, storageState saved");
  } catch (e) {
    console.error("[setup-auth] login failed:", e);
    throw e;
  } finally {
    await browser.close();
  }
}
