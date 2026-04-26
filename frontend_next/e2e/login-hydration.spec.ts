import { expect, test } from "@playwright/test";

test("login page does not fall back to native form submission before hydration", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false, locale: "zh-CN" });
  const page = await context.newPage();

  try {
    await page.goto("http://127.0.0.1:3000/login");

    const emailInput = page.locator("#login-email");
    const passwordInput = page.locator("#login-password");
    const submitButton = page.getByRole("button", { name: "继续登录" });

    await emailInput.fill("prehydrate@example.com");
    await passwordInput.fill("E2eTest123!");

    await expect(submitButton).toBeDisabled();
    await passwordInput.press("Enter");
    await expect(page).toHaveURL(/\/login$/);
  } finally {
    await context.close();
  }
});

test("login reaches dashboard after hydration", async ({ page, request }) => {
  const email = `pw-login-${Date.now()}@e2e.test`;
  const password = "E2eTest123!";

  const registerResponse = await request.post("http://127.0.0.1:3000/api/auth/register", {
    data: { email, password, full_name: "Playwright Login User" },
  });

  expect(registerResponse.ok(), await registerResponse.text()).toBeTruthy();

  await page.goto("/login");

  const emailInput = page.locator("#login-email");
  const passwordInput = page.locator("#login-password");
  const submitButton = page.getByRole("button", { name: "继续登录" });

  await page.waitForLoadState("networkidle");
  await expect(submitButton).toBeEnabled();
  await emailInput.fill(email);
  await passwordInput.fill(password);
  await submitButton.click();

  await expect(page).toHaveURL(/\/dashboard$/);
});
