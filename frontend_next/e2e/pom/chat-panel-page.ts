import { type Page, expect } from "@playwright/test";

export class ChatPanelPage {
  constructor(private page: Page) {}

  async sendMessage(text: string) {
    await this.ask(text);
  }

  async ask(question: string, mode?: "rag" | "search" | "chat") {
    if (mode) {
      await this.setMode(mode);
    }
    const composerInput = this.page.locator('[data-testid="workspace-chat-composer"]');
    await composerInput.waitFor({ timeout: 10_000 });
    await composerInput.fill(question);
    await this.page.locator('[data-testid="workspace-chat-send"]').click();
  }

  async setMode(mode: "rag" | "search" | "chat") {
    await this.page.locator('[data-testid="workspace-chat-mode-button"]').click();
    await this.page.locator(`[data-testid="workspace-chat-mode-${mode}"]`).click();
  }

  async switchToWebSearchMode() {
    await this.setMode("search");
  }

  async waitForResponse(timeout = 30_000) {
    await this.page.waitForSelector(
      '[data-testid="chat-message"][data-role="assistant"][data-pending="false"]',
      { timeout }
    );
  }

  async waitForAnswer(timeoutMs = 120_000) {
    await this.page
      .locator('[data-testid="chat-message"][data-role="assistant"]')
      .waitFor({ timeout: timeoutMs });
    try {
      await this.page
        .locator('[data-testid="workspace-progress-card"]')
        .waitFor({ state: "detached", timeout: timeoutMs });
    } catch {
      // progress card may not appear for fast responses
    }
  }

  getMessages() {
    return this.page.locator("[data-testid='chat-message']").all();
  }

  getLastMessage() {
    return this.page.locator("[data-testid='chat-message']").last();
  }

  async lastAnswerText(): Promise<string> {
    const raw = await this.lastAnswerRawText();
    return raw.replace(/<\/?[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  }

  async lastAnswerRawText(): Promise<string> {
    const bubble = this.page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]'
    ).last();
    return bubble.innerText();
  }

  async lastAnswerHtml(): Promise<string> {
    const bubble = this.page.locator(
      '[data-testid="chat-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]'
    ).last();
    return bubble.innerHTML();
  }

  getCitationButton() {
    return this.page.locator("[data-testid='citation-button']").first();
  }

  async citationCount(): Promise<number> {
    return this.page.locator('[data-testid="workspace-citation"]').count();
  }
}
