import { type Page, expect } from "@playwright/test";

export class ApiAccessPage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/api-access`);
    await this.page.waitForLoadState("networkidle");
  }

  async expectApiKeyListVisible() {
    await expect(
      this.page.getByRole("heading", { name: /已创建密钥|Created keys/i })
    ).toBeVisible();
  }

  async expectEmptyState() {
    await expect(this.page.getByText(/还没有 API 密钥/)).toBeVisible();
  }

  async createApiKey(name: string) {
    await this.page.getByLabel(/密钥名称|Key name/i).fill(name);
    await this.page.getByRole("button", { name: /创建密钥/i }).click();
  }

  async expectPlaintextShown() {
    await expect(this.page.getByText(/明文只会返回这一次/)).toBeVisible();
    await expect(this.page.locator("pre")).toBeVisible();
    await expect(this.page.locator("pre")).not.toBeEmpty();
  }

  keyItem(name: string) {
    return this.page.locator('[data-testid="api-key-item"]').filter({ hasText: name });
  }

  async expectKeyItemVisible(name: string) {
    await expect(this.keyItem(name)).toBeVisible();
    await expect(this.keyItem(name).getByText(/RPM/)).toBeVisible();
    await expect(this.keyItem(name).getByText(/生效中/)).toBeVisible();
  }

  async revokeKey(name: string) {
    await this.keyItem(name).getByRole("button", { name: /^撤销$/ }).click();
  }

  async expectKeyItemGone(name: string) {
    await expect(this.keyItem(name)).toHaveCount(0);
  }
}
