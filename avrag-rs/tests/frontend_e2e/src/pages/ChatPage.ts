import { Page, Locator, expect } from "@playwright/test";

export class ChatPage {
  readonly page: Page;
  readonly composerInput: Locator;
  readonly sendButton: Locator;
  readonly messages: Locator;
  readonly citations: Locator;
  readonly progressCard: Locator;
  readonly modeButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.composerInput = page.locator('[data-testid="workspace-chat-composer"]');
    this.sendButton = page.locator('[data-testid="workspace-chat-send"]');
    this.messages = page.locator('[data-testid="workspace-message"]');
    this.citations = page.locator('[data-testid="workspace-citation"]');
    this.progressCard = page.locator('[data-testid="workspace-progress-card"]');
    this.modeButton = page.locator('[data-testid="workspace-chat-mode-button"]');
  }

  async goto(workspaceId: string) {
    await this.page.goto(`/dashboard/${workspaceId}`);
    await this.page.waitForLoadState("networkidle");
  }

  async setMode(mode: "rag" | "search" | "chat") {
    await this.modeButton.click();
    await this.page.locator(`[data-testid="workspace-chat-mode-${mode}"]`).click();
  }

  async ask(question: string, mode?: "rag" | "search" | "chat") {
    if (mode) {
      await this.setMode(mode);
    }
    await this.composerInput.waitFor({ timeout: 10_000 });
    await this.composerInput.fill(question);
    await this.sendButton.click();
  }

  async waitForAnswer(timeoutMs = 120_000) {
    await this.page
      .locator('[data-testid="workspace-message"][data-role="assistant"]')
      .waitFor({ timeout: timeoutMs });
    try {
      await this.progressCard.waitFor({ state: "detached", timeout: timeoutMs });
    } catch {
      // progress card may not appear for fast responses
    }
  }

  async lastAnswerText(): Promise<string> {
    const raw = await this.lastAnswerRawText();
    // If the backend returns HTML (e.g. slide deck), strip tags so the judge
    // sees plain text instead of raw markup.
    return raw.replace(/\u003c\/?[^\u003e]+\u003e/g, " ").replace(/\s+/g, " ").trim();
  }

  async lastAnswerRawText(): Promise<string> {
    const bubble = this.page.locator(
      '[data-testid="workspace-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]'
    ).last();
    return bubble.innerText();
  }

  async lastAnswerHtml(): Promise<string> {
    const bubble = this.page.locator(
      '[data-testid="workspace-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]'
    ).last();
    return bubble.innerHTML();
  }

  async citationCount(): Promise<number> {
    return this.citations.count();
  }
}
