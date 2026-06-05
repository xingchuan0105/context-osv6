import { Page, Locator } from "@playwright/test";

export class ChatPage {
  readonly page: Page;
  readonly input: Locator;
  readonly sendButton: Locator;
  readonly messages: Locator;
  readonly citations: Locator;

  constructor(page: Page) {
    this.page = page;
    this.input = page.locator('[data-testid="chat-input"]').or(page.locator("textarea")).first();
    this.sendButton = page.locator('[data-testid="chat-send"]').or(page.locator("button", { hasText: /send/i })).first();
    this.messages = page.locator('[data-testid="chat-message"]').or(page.locator("[class*='message']"));
    this.citations = page.locator('[data-testid="citation"]').or(page.locator("a[href*='cite']"));
  }

  async goto(notebookId?: string) {
    const url = notebookId ? `/chat?notebook=${notebookId}` : "/chat";
    await this.page.goto(url);
    await this.page.waitForLoadState("networkidle");
  }

  async ask(question: string) {
    await this.input.fill(question);
    await this.sendButton.click();
    // Wait for answer stream to complete (no more loading spinner)
    await this.page.waitForSelector("[data-testid='chat-loading']", { state: "hidden", timeout: 60_000 }).catch(() => {});
    // Wait for at least one message to appear
    await this.messages.first().waitFor({ timeout: 60_000 });
  }

  async lastAnswer(): Promise<string> {
    const last = this.messages.last();
    return last.innerText();
  }

  async citationCount(): Promise<number> {
    return this.citations.count();
  }

  async clickFirstCitation() {
    await this.citations.first().click();
  }
}
