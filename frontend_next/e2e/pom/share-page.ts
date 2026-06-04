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
      await this.page.waitForFunction(
        () => {
          const urlEl = document.querySelector('[style*="font-family: ui-monospace"]');
          return urlEl && urlEl.textContent && urlEl.textContent.includes("/shared/kb/");
        },
        { timeout: 10_000 }
      );
    }
  }

  async copyShareLink(): Promise<string> {
    // 等待 share URL 出现
    const urlLocator = this.page.locator('[style*="font-family: ui-monospace"]');
    await urlLocator.waitFor({ timeout: 5_000 });
    const url = await urlLocator.textContent();
    return url?.trim() ?? "";
  }
}
