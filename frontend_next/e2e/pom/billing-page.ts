import { type Page, expect } from "@playwright/test";

export class PricingPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.goto("/pricing");
  }

  async expectVisible() {
    await expect(this.page.getByRole("heading", { name: /选择适合你的方案/ })).toBeVisible();
    await expect(this.page.getByText("Plus")).toBeVisible();
    await expect(this.page.getByText("Free")).toBeVisible();
    await expect(this.page.getByText("Pro")).toBeVisible();
  }

  async clickUpgrade(plan: "plus" | "pro") {
    await this.page
      .getByRole("button", { name: new RegExp(`升级 ${plan === "plus" ? "Plus" : "Pro"}`) })
      .click();
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
