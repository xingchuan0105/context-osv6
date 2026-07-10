import { type Page, expect } from "@playwright/test";

export class AdminPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/admin");
    await this.page.waitForLoadState("networkidle");
  }

  async expectLoaded() {
    await expect(
      this.page.getByRole("heading", { name: /组织|Accounts/i })
    ).toBeVisible();
  }

  async navigateToUsers() {
    await this.page.getByRole("link", { name: /用户|Users/i }).click();
    await this.page.waitForURL(/\/admin\/users$/);
  }

  async navigateToAccounts() {
    await this.page.getByRole("link", { name: /^(组织|Accounts)$/i }).first().click();
    await this.page.waitForURL(/\/admin\/?$/);
  }

  async expectUserTableVisible() {
    await expect(this.page.getByRole("table")).toBeVisible();
  }
}
