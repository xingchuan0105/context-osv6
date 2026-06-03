import { type Page, expect } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  async sendMessage(text: string) {
    const input = this.page.getByPlaceholder(/围绕当前资料继续研究|Ask a question/);
    await input.fill(text);
    await this.page.getByRole("button", { name: /发送|Send/ }).click();
  }

  async waitForResponse(timeout = 30_000) {
    await this.page.waitForSelector("[data-testid='message-done']", { timeout });
  }

  getMessages() {
    return this.page.locator("[data-testid='chat-message']").all();
  }

  getLastMessage() {
    return this.page.locator("[data-testid='chat-message']").last();
  }

  async switchToWebSearchMode() {
    await this.page.getByRole("button", { name: /联网搜索|Web Search/ }).click();
    await expect(this.page.locator("[data-testid='mode-indicator']")).toContainText(/search|联网/i);
  }

  async switchToHistoryTab() {
    await this.page.getByRole("button", { name: /历史|History/ }).click();
  }

  async uploadFile(filePath: string) {
    const input = this.page.locator("input[type='file']");
    await input.setInputFiles(filePath);
    await expect(this.page.locator("[data-testid='upload-done']")).toBeVisible({ timeout: 10_000 });
  }

  async waitForIngestionComplete(timeout = 60_000) {
    await expect(this.page.locator("[data-testid='ingestion-status']")).toHaveText(/completed|已完成/, { timeout });
  }

  getCitationButton() {
    return this.page.locator("[data-testid='citation-button']").first();
  }
}
