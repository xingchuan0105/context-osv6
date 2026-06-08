import { type Page, expect } from "@playwright/test";

export class DashboardPage {
  constructor(private page: Page) {}

  async createWorkspace() {
    await this.page.locator('[data-testid="dashboard-create-workspace"]').click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
  }

  async openWorkspace(name: string) {
    const card = this.page.locator(".dashboard-workspace-card", {
      has: this.page.getByText(name, { exact: true }),
    });
    await card.locator(".dashboard-workspace-card-link").click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);
    await this.page.locator("#workspace-title").waitFor({ state: "visible", timeout: 10_000 });
  }

  getWorkspaceList() {
    return this.page.locator("[data-testid='notebook-list']");
  }
}
