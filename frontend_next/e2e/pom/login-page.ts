import { type Page, type Locator, expect } from "@playwright/test";

export class LoginPage {
  readonly emailInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;
  readonly errorMessage: Locator;

  constructor(private page: Page) {
    this.emailInput = page.locator("#login-email");
    this.passwordInput = page.locator("#login-password");
    this.submitButton = page.getByRole("button", { name: /继续登录/ });
    this.errorMessage = page.locator("[data-testid='login-error']");
  }

  async goto() {
    await this.page.goto("/login");
    await expect(this.page.locator("form").first()).toBeVisible();
  }

  async login(email: string, password: string) {
    await this.goto();
    await this.emailInput.fill(email);
    await this.passwordInput.fill(password);
    await expect(this.submitButton).toBeEnabled();
    await this.submitButton.click();
    await this.page.waitForURL(/\/dashboard$/, { timeout: 15_000 });
  }
}
