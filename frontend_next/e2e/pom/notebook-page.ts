import { type Page, expect } from "@playwright/test";

export class NotebookPage {
  private lastCreatedName: string | null = null;

  constructor(private page: Page) {}

  /** 从 /dashboard 创建 workspace 并通过 inline 编辑设置名称 */
  async createNotebook(name: string) {
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");

    // 点击 dashboard 上的新建按钮（自动创建并跳转到新 workspace）
    await this.page
      .getByRole("button", { name: /新建工作区|New workspace/i })
      .click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);

    // 等待顶部标题编辑触发器可见
    const titleTrigger = this.page.locator("#workspace-title");
    await titleTrigger.waitFor({ state: "visible", timeout: 10_000 });

    // 进入编辑模式、填写新名称、提交
    await titleTrigger.click();
    await titleTrigger.fill(name);
    await titleTrigger.press("Enter");

    // 提交后触发器恢复为 button，验证文本已更新
    await expect(this.page.locator("#workspace-title")).toHaveText(name);
    this.lastCreatedName = name;
  }

  /** 要求在 workspace 页面（已定位到目标 workspace） */
  async renameNotebook(newName: string) {
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
    let cardLocator = this.page.locator(".dashboard-workspace-card").first();

    if (targetName) {
      cardLocator = this.page.locator(".dashboard-workspace-card", {
        has: this.page.getByText(targetName),
      });
    }

    // 打开 action menu（三点按钮）
    await cardLocator.locator(".dashboard-menu-trigger").click();

    // 处理 window.confirm 确认框
    this.page.once("dialog", (dialog) => dialog.accept());

    await this.page.getByRole("menuitem", { name: /删除|Delete/i }).click();

    await expect(cardLocator).toBeHidden();
    this.lastCreatedName = null;
  }
}
