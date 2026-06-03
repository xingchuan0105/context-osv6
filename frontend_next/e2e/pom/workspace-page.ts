import { type Page, expect } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  async sendMessage(text: string) {
    const input = this.page.getByPlaceholder(/围绕当前资料继续研究|Ask a question/);
    await input.fill(text);
    await this.page.getByRole("button", { name: /发送|Send/ }).click();
  }

  async waitForResponse(timeout = 30_000) {
    // 等待最后一条 assistant 消息完成（pending=false）
    await this.page.waitForSelector('[data-testid="chat-message"][data-role="assistant"][data-pending="false"]', { timeout });
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
    // 上传完成后 source item 会出现在列表中，由后续 waitForIngestionComplete 验证
  }

  async waitForIngestionComplete(timeout?: number) {
    // P2: timeout 从环境变量读取，CI 中可覆盖
    const effectiveTimeout = timeout ?? parseInt(process.env.E2E_INGESTION_TIMEOUT || "60000", 10);
    // 等待任意 source item 的 data-status 变为 completed
    await this.page.waitForSelector('[data-testid="ingestion-status"][data-status="completed"]', { timeout: effectiveTimeout });
  }

  getCitationButton() {
    return this.page.locator("[data-testid='citation-button']").first();
  }
}
