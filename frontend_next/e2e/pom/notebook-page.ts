import { Page } from "@playwright/test";

// TODO(Phase 3): 当前为占位符，需根据实际 UI 完善 selector 和交互逻辑
export class NotebookPage {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  async createNotebook(name: string) {
    // TODO: 实现 UI 流程——点击"新建 Notebook"→填写名称→提交
    await this.page.goto("/dashboard");
    await this.page.waitForLoadState("networkidle");
  }

  async uploadDocument(fixturePath: string) {
    // TODO: 实现 UI 流程——打开添加内容源 dialog→选择文件→确认上传
  }
}
