import { type Page, expect } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  async sendMessage(text: string) {
    const input = this.page.getByPlaceholder(/输入 \/ 选择模式|Type \/ to choose/);
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
    // 点击模式按钮展开菜单
    await this.page.getByRole("button", { name: /对话模式|Chat mode/i }).click();
    // 选择网络搜索模式
    await this.page.getByRole("button", { name: /网络搜索|web_search/i }).click();
  }

  async switchToHistoryTab() {
    // 桌面端 history rail 默认显示，无需点击切换
    await this.page.waitForSelector("[data-testid='desktop-history-rail']", { state: "visible" });
  }

  async uploadFile(filePath: string) {
    // 打开"添加内容源"dialog（file input 在 dialog 内，必须先打开）
    await this.page.getByRole("button", { name: /添加内容源|New source|上传文件/i }).click();
    const input = this.page.locator('input[type="file"]');
    await input.setInputFiles(filePath);
    // 上传完成后 dialog 自动关闭，source item 出现在列表中
  }

  async waitForIngestionComplete(timeout?: number) {
    // P2: timeout 从环境变量读取，CI 中可覆盖
    const effectiveTimeout = timeout ?? parseInt(process.env.E2E_INGESTION_TIMEOUT || "120000", 10);
    // 等待任意 source item 的 data-status 变为 completed 或 ready
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
      { timeout: effectiveTimeout }
    );
  }

  getCitationButton() {
    return this.page.locator("[data-testid='citation-button']").first();
  }
}
