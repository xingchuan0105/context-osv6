import { test, expect } from "@playwright/test";
import { LoginPage } from "../src/pages/LoginPage";

test("frontend loads and shows expected elements", async ({ page }) => {
  const login = new LoginPage(page);
  await login.goto();
  await login.expectLoaded();

  // Minimal sanity: page title or body should not be empty
  const body = await page.locator("body").innerText();
  expect(body.length).toBeGreaterThan(0);
});
