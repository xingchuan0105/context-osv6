import { type Page } from "@playwright/test";

export class SharePage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/share`);
  }

  async enableShare() {
    // toggle switch: role="switch"
    const toggle = this.page.getByRole("switch");
    const isChecked = await toggle.getAttribute("aria-checked");
    if (isChecked !== "true") {
      await toggle.click();
      // 等待 share token 生成（URL 从空变为有值）
      // TODO: 待 UI 给 share URL 元素加 data-testid="share-link" 后替换为稳定 locator
      await this.page.waitForFunction(
        () => {
          const urlEl = Array.from(
            document.querySelectorAll("[data-testid='share-link'], [style*='font-family: ui-monospace']")
          ).find((el) => el.textContent?.includes("/shared/kb/"));
          return !!urlEl;
        },
        { timeout: 10_000 }
      );
    }
  }

  async copyShareLink(): Promise<string> {
    // 等待 share URL 出现
    // TODO: 待 UI 给 share URL 元素加 data-testid="share-link" 后优先使用
    const urlLocator = this.page.locator("[data-testid='share-link'], [style*='font-family: ui-monospace']");
    await urlLocator.waitFor({ timeout: 5_000 });
    const url = await urlLocator.textContent();
    return url?.trim() ?? "";
  }
}
