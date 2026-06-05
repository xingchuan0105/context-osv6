import { Page, Locator } from "@playwright/test";

export class LoginPage {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  async goto() {
    await this.page.goto("/");
  }

  async expectLoaded() {
    await this.page.waitForLoadState("networkidle");
  }
}
