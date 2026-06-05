import { Page } from "@playwright/test";

export class NotebookPage {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  async createNotebook(name: string) {
    // Placeholder: adjust selectors after inspecting actual UI
    await this.page.goto("/notebooks");
    await this.page.waitForLoadState("networkidle");
  }

  async uploadDocument(fixturePath: string) {
    // Placeholder
  }
}
