import { type Page, expect } from "@playwright/test";

export class WorkspacePage {
  private lastCreatedName: string | null = null;

  constructor(private page: Page) {}

  /** 从 /dashboard 创建 workspace 并通过 inline 编辑设置名称 */
  async createWorkspace(name: string) {
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");

    await this.page.locator('[data-testid="dashboard-create-workspace"]').click();
    await this.page.waitForURL(/\/dashboard\/[^/]+$/);

    await this.page.locator('[data-testid="workspace-top-bar"]').waitFor({ state: "visible" });
    const titleTrigger = this.page.locator("#workspace-title");
    await titleTrigger.waitFor({ state: "visible", timeout: 10_000 });

    await titleTrigger.click();
    await titleTrigger.fill(name);
    await titleTrigger.press("Enter");

    await expect(this.page.locator("#workspace-title")).toHaveText(name);
    this.lastCreatedName = name;
  }

  /** 要求在 workspace 页面（已定位到目标 workspace） */
  async renameWorkspace(newName: string) {
    await this.page.locator('[data-testid="workspace-top-bar"]').waitFor({ state: "visible" });
    const titleTrigger = this.page.locator("#workspace-title");
    await titleTrigger.waitFor({ state: "visible", timeout: 10_000 });

    await titleTrigger.click();
    await titleTrigger.fill(newName);
    await titleTrigger.press("Enter");

    await expect(this.page.locator("#workspace-title")).toHaveText(newName);
    this.lastCreatedName = newName;
  }

  /** 从 /dashboard 找到最近创建的 workspace 并删除 */
  async deleteWorkspace() {
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");

    const targetName = this.lastCreatedName;
    if (!targetName) {
      throw new Error(
        "deleteWorkspace called but no workspace was created in this test. Call createWorkspace first.",
      );
    }
    const cardLocator = this.page.locator('[data-testid="dashboard-workspace-item"]', {
      has: this.page.getByText(targetName, { exact: true }),
    });

    await cardLocator.locator(".dashboard-menu-trigger").click();

    this.page.once("dialog", (dialog) => dialog.accept());

    await this.page.getByRole("menuitem", { name: /删除|Delete/i }).click();

    await expect(cardLocator).toBeHidden();
    this.lastCreatedName = null;
  }

  /** 桌面端 history rail 默认显示，只需等待其可见 */
  async waitForHistoryTabVisible() {
    await this.page.waitForSelector("[data-testid='desktop-history-rail']", { state: "visible" });
  }

  /** @deprecated Product removed query-library panel; use history rail helpers. */
  getQueryLibraryPanel() {
    return this.page.getByTestId("desktop-history-rail");
  }

  getHistoryRail() {
    return this.page.getByTestId("desktop-history-rail");
  }

  async clickHistoryItemContaining(text: string) {
    const rail = this.getHistoryRail();
    await rail.waitFor({ state: "visible" });
    await rail.locator('[data-testid="history-item"]').filter({ hasText: text }).first().click();
  }

  async clickQueryLibraryItem(text: string) {
    await this.clickHistoryItemContaining(text);
  }

  async uploadFile(filePath: string) {
    // 打开"添加内容源"dialog（file input 在 dialog 内，必须先打开）
    await this.page.getByRole("button", { name: /添加内容源|New source|上传文件/i }).click();
    const input = this.page.locator('input[type="file"]');
    await input.setInputFiles(filePath);
    // 等待上传状态至少变为 pending/processing（避免空转 ingestion 等待）
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="pending"], [data-testid="ingestion-status"][data-status="processing"], [data-testid="ingestion-status"][data-status="completed"]',
      { timeout: 30_000 },
    );
  }

  async waitForIngestionComplete(timeout?: number) {
    // P2: timeout 从环境变量读取，CI 中可覆盖
    // Local REUSE worker may share queue with leftover tasks; 180s is safer than 120s.
    const effectiveTimeout = timeout ?? parseInt(process.env.E2E_INGESTION_TIMEOUT || "180000", 10);
    // 等待任意 source item 的 data-status 变为 completed 或 ready
    await this.page.waitForSelector(
      '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
      { timeout: effectiveTimeout },
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
