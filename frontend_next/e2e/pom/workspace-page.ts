import { type Page } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  /** 桌面端 history rail 默认显示，只需等待其可见 */
  async waitForHistoryTabVisible() {
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
}
