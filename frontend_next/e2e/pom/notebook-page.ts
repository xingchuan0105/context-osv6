import { type Page, expect } from "@playwright/test";

export class NotebookPage {
  private lastCreatedName: string | null = null;

  constructor(private page: Page) {}

  /** 从 /dashboard 创建 workspace 并通过 inline 编辑设置名称 */
  async createNotebook(name: string) {
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");

    await this.page.locator('[data-testid="dashboard-create-workspace"]').click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);

    await this.page.locator('[data-testid="workspace-top-bar"]').waitFor({ state: "visible" });
    const titleTrigger = this.page.locator("#workspace-title");
    await titleTrigger.waitFor({ state: "visible", timeout: 10_000 });

    await titleTrigger.click();
    await titleTrigger.fill(name);
    await titleTrigger.press("Enter");

    await expect(this.page.locator("#workspace-title")).toHaveText(name);
    this.lastCreatedName = name;
  }

  /** 要求在 workspace 页面（已定位到目标 workspace） */
  async renameNotebook(newName: string) {
    await this.page.locator('[data-testid="workspace-top-bar"]').waitFor({ state: "visible" });
    const titleTrigger = this.page.locator("#workspace-title");
    await titleTrigger.waitFor({ state: "visible", timeout: 10_000 });

    await titleTrigger.click();
    await titleTrigger.fill(newName);
    await titleTrigger.press("Enter");

    await expect(this.page.locator("#workspace-title")).toHaveText(newName);
    this.lastCreatedName = newName;
  }

  /** 从 /dashboard 找到最近创建的 workspace 并删除 */
  async deleteNotebook() {
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");

    const targetName = this.lastCreatedName;
    if (!targetName) {
      throw new Error("deleteNotebook called but no notebook was created in this test. Call createNotebook first.");
    }
    const cardLocator = this.page.locator('[data-testid="dashboard-workspace-item"]', {
      has: this.page.getByText(targetName, { exact: true }),
    });

    await cardLocator.locator(".dashboard-menu-trigger").click();

    this.page.once("dialog", (dialog) => dialog.accept());

    await this.page.getByRole("menuitem", { name: /删除|Delete/i }).click();

    await expect(cardLocator).toBeHidden();
    this.lastCreatedName = null;
  }
}
