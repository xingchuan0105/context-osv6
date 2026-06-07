import { type Page, expect } from "@playwright/test";

export class AnalyzePage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/analyze`);
    await this.page.waitForURL(/\/dashboard\/[^/]+\/share/);
  }

  async expectChartVisible() {
    // TODO: 当前 share#insights 页面使用纯 CSS 条形图渲染趋势，没有 <canvas>
    // 或 data-testid="analyze-chart" 等稳定 selector。待 UI 侧补充后替换为精确断言。
    await expect(this.page.locator("section#insights")).toBeVisible();
  }

  async expectInsightVisible() {
    const insights = this.page.locator("section#insights");
    await expect(insights).toBeVisible();
    await expect(insights.getByRole("heading").first()).toBeVisible();
  }
}
