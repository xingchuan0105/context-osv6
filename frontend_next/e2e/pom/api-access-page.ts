import { type Page, expect } from "@playwright/test";

export class ApiAccessPage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    // API 设置已合并进分享中心（API = 一种分享方法）；入口在「API 访问」标签页。
    await this.page.goto(`/dashboard/${workspaceId}/share`);
    await this.page.waitForLoadState("networkidle");
    await this.page.getByRole("button", { name: "API 访问" }).click();
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
    //「先创建密钥」(copy-agent-pack, disabled 占位) 也会命中 /创建密钥/，须精确匹配提交键。
    await this.page.getByRole("button", { name: "创建密钥", exact: true }).click();
  }

  async expectPlaintextShown() {
    await expect(this.page.getByText(/明文只会返回这一次/)).toBeVisible();
    // agent-pack-preview 也是 <pre>（常驻），新密钥明文块须排除它。
    const plaintext = this.page.locator('pre:not([data-testid="agent-pack-preview"])');
    await expect(plaintext).toBeVisible();
    await expect(plaintext).not.toBeEmpty();
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
