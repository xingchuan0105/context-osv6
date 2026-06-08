import { type Page, expect } from "@playwright/test";

export class SettingsPage {
  constructor(private page: Page) {}

  async gotoBillingTab() {
    await this.page.goto("/settings?tab=billing");
    await this.page.waitForLoadState("networkidle");
  }

  async expectBillingTabActive() {
    await expect(
      this.page.getByRole("link", { name: /账单|Billing/i }),
    ).toHaveAttribute("aria-current", "page");
  }

  async expectBillingSectionLoaded() {
    await expect(
      this.page.getByRole("heading", { name: /账单与计划|Billing and plan/i }),
    ).toBeVisible();
    await expect(
      this.page.getByRole("button", { name: /管理计划|Manage plan/i }),
    ).toBeVisible();
    await expect(this.page.getByText(/当前计划|Current plan/i).first()).toBeVisible();
    await expect(
      this.page.getByRole("heading", { name: /^用量$|^Usage$/i }),
    ).toBeVisible();
    await expect(
      this.page.getByRole("heading", { name: /可用方案|Available plans/i }),
    ).toBeVisible();
    await expect(
      this.page.getByText(/正在加载账单信息|Loading billing information/i),
    ).toHaveCount(0);
  }

  async expectUsageMeterVisible() {
    await expect(this.page.getByTestId("usage-meter")).toBeVisible();
    await expect(this.page.getByRole("progressbar").first()).toBeVisible();
  }

  async expectPlanDisplayVisible() {
    await expect(this.page.getByTestId("plan-display")).toBeVisible();
    await expect(this.page.getByText(/当前计划|Current plan/i).first()).toBeVisible();
  }
}
