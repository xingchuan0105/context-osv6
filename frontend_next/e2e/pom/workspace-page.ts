import { type Page } from "@playwright/test";

export class WorkspacePage {
  constructor(private page: Page) {}

  /** 桌面端 history rail 默认显示，只需等待其可见 */
  async waitForHistoryTabVisible() {
    await this.page.waitForSelector("[data-testid='desktop-history-rail']", { state: "visible" });
  }

  getQueryLibraryPanel() {
    return this.page.getByTestId("query-library-panel");
  }

  async clickQueryLibraryItem(text: string) {
    const panel = this.getQueryLibraryPanel();
    await panel.waitFor({ state: "visible" });
    await panel.getByText(text, { exact: true }).click();
  }

  async uploadFile(filePath: string) {
    // 打开"添加内容源"dialog（file input 在 dialog 内，必须先打开）
    await this.page.getByRole("button", { name: /添加内容源|New source|上传文件/i }).click();
    const input = this.page.locator('input[type="file"]');
    await input.setInputFiles(filePath);
    // 等待上传状态至少变为 pending/processing（避免空转 ingestion 等待）
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="pending"], [data-testid="ingestion-status"][data-status="processing"], [data-testid="ingestion-status"][data-status="completed"]',
      { timeout: 30_000 }
    );
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

  /** 返回最近完成 ingest 的 document id（依赖 data-document-id） */
  async getLatestCompletedDocumentId(): Promise<string> {
    const row = this.page
      .locator(
        '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
      )
      .first();
    await row.waitFor({ state: "visible", timeout: 30_000 });
    const id = await row.getAttribute("data-document-id");
    if (!id) {
      throw new Error("ingestion row missing data-document-id");
    }
    return id;
  }
}
