import { test, expect } from "@playwright/test";
import { TEST_USER } from "../fixtures/test-user";

test.describe("Auth Failure Cases", () => {
  test("login shows error for wrong password", async ({ page }) => {
    await page.goto("/login");
    await page.locator("#login-email").fill(TEST_USER.email);
    await page.locator("#login-password").fill("WrongPassword123!");
    await page.getByRole("button", { name: /继续登录|Sign in/i }).click();

    // 期望看到错误提示（alert 或 banner）
    await expect(page.locator(".app-notice-banner, [role='alert']")).toBeVisible();
  });

  test("login prevents submit with empty fields", async ({ page }) => {
    await page.goto("/login");

    // 空邮箱 + 空密码，提交按钮应为 disabled 或点击无效
    const submitButton = page.getByRole("button", { name: /继续登录|Sign in/i });
    const isDisabled = await submitButton.isDisabled().catch(() => false);

    if (!isDisabled) {
      // 如果按钮不禁用，点击后应仍在登录页（未跳转）
      await submitButton.click();
      await page.waitForTimeout(500);
      expect(page.url()).toContain("/login");
    }
  });

  test("register prevents submit with mismatched passwords", async ({ page }) => {
    await page.goto("/register");
    await page.locator("#register-email").fill("mismatch-test@test.local");
    await page.locator("#register-password").fill("Password123!");
    await page.locator("#register-password-confirm").fill("Different456!");
    await page.locator("#register-name").fill("Mismatch Test");

    const submitButton = page.getByRole("button", { name: /创建账号|Create account/i });
    const isDisabled = await submitButton.isDisabled().catch(() => false);

    if (!isDisabled) {
      await submitButton.click();
      await page.waitForTimeout(500);
      // 应仍在注册页
      expect(page.url()).toContain("/register");
    }
  });
});
