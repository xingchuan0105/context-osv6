import { type Page, expect } from "@playwright/test";

export class DesktopWorkbench {
  constructor(private page: Page) {}

  async goToDashboard() {
    await this.page.goto("http://tauri.localhost/dashboard");
    await this.waitForDashboard();
  }

  async waitForDashboard() {
    await this.page.waitForURL((url) => url.hostname === "tauri.localhost" && url.pathname.startsWith("/dashboard"));
    await this.page.getByTestId("dashboard-create-workspace").waitFor({ state: "visible", timeout: 30_000 });
  }

  async createWorkspace() {
    await this.page.getByTestId("dashboard-create-workspace").click();
    await this.page.waitForURL(
      (url) =>
        url.hostname === "tauri.localhost" &&
        url.pathname === "/dashboard/_placeholder" &&
        url.searchParams.has("ws"),
      { timeout: 30_000 },
    );
    await this.page.getByTestId("workspace-top-bar").waitFor({ state: "visible", timeout: 30_000 });
  }

  async uploadFile(filePath: string) {
    await this.page.getByRole("button", { name: /添加资料|New source|上传文件/i }).click();
    const input = this.page.locator('input[type="file"]');
    await input.setInputFiles(filePath);
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="pending"], [data-testid="ingestion-status"][data-status="processing"], [data-testid="ingestion-status"][data-status="completed"]',
      { timeout: 30_000 },
    );
  }

  async waitForIngestionComplete(timeout = 180_000) {
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
      { timeout },
    );
  }

  async expectWorkspaceUrlHasWorkspaceId() {
    const url = this.page.url();
    const parsed = new URL(url);
    expect(parsed.searchParams.has("ws")).toBe(true);
  }

  async sendMessage(query: string) {
    const composer = this.page.getByTestId("workspace-chat-composer");
    await composer.fill(query);
    await this.page.getByTestId("workspace-chat-send").click();
  }

  async waitForChatError(pattern: RegExp, timeout = 30_000) {
    const alert = this.page.locator('[data-testid="workspace-chat-pane"] [role="alert"]');
    await alert.waitFor({ state: "visible", timeout });
    await expect(alert).toContainText(pattern);
  }

  /** Wait for any chat error alert and return its text (for "error but not X" assertions). */
  async waitForAnyChatError(timeout = 30_000): Promise<string> {
    const alert = this.page.locator('[data-testid="workspace-chat-pane"] [role="alert"]');
    await alert.waitFor({ state: "visible", timeout });
    return (await alert.textContent()) ?? "";
  }

  async waitForAssistantMessage(timeout = 120_000) {
    await this.page
      .locator('[data-testid="workspace-chat-pane"] [data-testid="chat-message-assistant"]')
      .first()
      .waitFor({ state: "visible", timeout });
  }
}
