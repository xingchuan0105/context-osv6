import { type Page, expect } from "@playwright/test";

export class AnalyzePage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/analyze`);
    // /analyze 实际会重定向到 /share#insights，等待重定向完成
    await this.page.waitForURL(/\/dashboard\/[^/]+\/share/);
  }

  async expectChartVisible() {
    await expect(this.page.locator('[data-testid="analyze-chart"]')).toBeVisible();
  }

  async expectInsightVisible() {
    const insights = this.page.locator("section#insights");
    await expect(insights).toBeVisible();
    await expect(insights.getByRole("heading").first()).toBeVisible();
  }
}
