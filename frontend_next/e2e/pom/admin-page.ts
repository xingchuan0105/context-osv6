import { type Page, expect } from "@playwright/test";

export class AdminPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/admin");
    await this.page.waitForLoadState("networkidle");
  }

  async expectLoaded() {
    // Post-org-removal: default /admin is personal accounts surface (中文「账户」).
    // Keep legacy「组织」/Accounts for older builds during transition.
    await expect(
      this.page.getByRole("heading", { name: /账户|组织|Accounts|Users|用户/i })
    ).toBeVisible({ timeout: 15_000 });
  }

  async navigateToUsers() {
    await this.page.getByRole("link", { name: /用户|Users/i }).click();
    await this.page.waitForURL(/\/admin\/users$/);
  }

  async navigateToAccounts() {
    await this.page
      .getByRole("link", { name: /^(账户|组织|Accounts)$/i })
      .first()
      .click();
    await this.page.waitForURL(/\/admin\/?$/);
  }

  async expectUserTableVisible() {
    await expect(this.page.getByRole("table")).toBeVisible();
  }
}
