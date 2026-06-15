import { type Page, expect } from "@playwright/test";

export class SharePage {
  constructor(private page: Page) {}

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}/share`);
    await this.page.locator('[data-testid="share-control-bar"]').waitFor({ state: "visible" });
  }

  async enableShare() {
    const toggle = this.page.locator('[data-testid="share-control-bar"] [role="switch"]');
    const isChecked = await toggle.getAttribute("aria-checked");
    if (isChecked !== "true") {
      await toggle.click();
      await this.page.locator('[data-testid="share-link"]').filter({ hasText: /\/shared\/kb\// }).waitFor({
        timeout: 10_000,
      });
    }
  }

  async copyShareLink(): Promise<string> {
    const urlLocator = this.page.locator('[data-testid="share-link"]').filter({ hasText: /\/shared\/kb\// });
    await urlLocator.waitFor({ timeout: 5_000 });
    const url = await urlLocator.textContent();
    return url?.trim() ?? "";
  }

  async inviteMember(email: string) {
    await this.page.locator('[data-testid="share-invite-email"]').fill(email);
    await this.page.locator('[data-testid="share-invite-send"]').click();
    await expect(this.page.locator('[data-testid="share-invite-member"]').first()).toBeVisible({
      timeout: 10_000,
    });
  }
}
