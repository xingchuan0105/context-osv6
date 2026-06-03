import { type Page, expect } from "@playwright/test";

export class DashboardPage {
  constructor(private page: Page) {}

  async createWorkspace() {
    await this.page.getByRole("button", { name: /新建工作区|New workspace/i }).click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
  }

  async openWorkspace(name: string) {
    await this.page.locator("text=" + name).first().click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
  }

  getWorkspaceList() {
    return this.page.locator("[data-testid='notebook-list']");
  }
}
