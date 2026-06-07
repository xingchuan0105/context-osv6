import { type Page, expect } from "@playwright/test";

export class ApiAccessPage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/api-access`);
    await this.page.waitForLoadState("networkidle");
  }

  async createApiKey(name: string) {
    await this.page.getByLabel(/密钥名称|Key name/i).fill(name);
    await this.page.getByRole("button", { name: /创建密钥/i }).click();
    await expect(
      this.page.locator(".app-inline-surface").filter({ hasText: name })
    ).toBeVisible();
  }

  async expectApiKeyListVisible() {
    await expect(
      this.page.getByRole("heading", { name: /已创建密钥|Created keys/i })
    ).toBeVisible();
  }
}
