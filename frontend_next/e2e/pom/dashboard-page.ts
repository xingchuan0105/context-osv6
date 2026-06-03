import { type Page, expect } from "@playwright/test";

export class DashboardPage {
  constructor(private page: Page) {}

  async createNotebook(name: string) {
    await this.page.getByRole("button", { name: /新建/ }).click();
    await this.page.getByPlaceholder(/名称/).fill(name);
    await this.page.getByRole("button", { name: /确认/ }).click();
    await expect(this.page.locator("text=" + name)).toBeVisible();
  }

  async openNotebook(name: string) {
    await this.page.locator("text=" + name).first().click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
  }

  getNotebookList() {
    return this.page.locator("[data-testid='notebook-list']");
  }
}
