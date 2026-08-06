import { type Page, expect } from "@playwright/test";

export class AnalyzePage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    // Canonical: Share center; /analyze redirects (PRODUCT_IA).
    await this.page.goto(`/dashboard/${workspaceId}/analyze`);
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
