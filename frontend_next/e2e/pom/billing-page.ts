import { type Page, expect } from "@playwright/test";

export class PricingPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/pricing");
    await expect(this.page.getByRole("heading", { name: /选择适合你的方案/ })).toBeVisible({
      timeout: 30_000,
    });
    await this.page.waitForLoadState("networkidle").catch(() => {});
  }

  async expectVisible() {
    await expect(this.page.getByRole("heading", { name: /选择适合你的方案/ })).toBeVisible();
    await expect(this.page.getByRole("heading", { name: "Plus" })).toBeVisible();
    await expect(this.page.getByRole("heading", { name: "Free" })).toBeVisible();
    await expect(this.page.getByRole("heading", { name: "Pro" })).toBeVisible();
  }

  async clickUpgrade(plan: "plus" | "pro") {
    const label = plan === "plus" ? "Plus" : "Pro";
    const button = this.page.getByRole("button", { name: new RegExp(`升级 ${label}`) });
    await expect(button).toBeVisible();
    await button.click();
  }
}

export class UsagePage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/settings/usage");
  }

  async expectVisible() {
    await expect(this.page.getByText(/用量与套餐/)).toBeVisible();
    await expect(this.page.getByText(/5 小时窗口/)).toBeVisible();
    await expect(this.page.getByText(/7 天窗口/)).toBeVisible();
  }
}

export class PaywallPage {
  constructor(private page: Page) {}

  async goto(reason: "5h" | "7d" = "5h") {
    await this.page.goto(`/upgrade/paywall?reason=${reason}`);
  }

  async expectVisible() {
    await expect(this.page.getByRole("dialog")).toBeVisible();
  }
}
