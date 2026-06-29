import { type Page, type Locator, expect } from "@playwright/test";

export class ChatPanelPage {
  constructor(private page: Page) {}

  /** 向后兼容的简写：不带模式参数的消息发送 */
  async sendMessage(text: string) {
    await this.ask(text);
  }

  /** 受控 textarea：fill 在第二轮对话后可能不触发 React onChange，需走 native setter。 */
  private async setComposerDraft(question: string) {
    const composerInput = this.page.locator('[data-testid="workspace-chat-composer"]');
    await composerInput.evaluate((el, value) => {
      const textarea = el as HTMLTextAreaElement;
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      setter?.call(textarea, value);
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new Event("change", { bubbles: true }));
    }, question);
  }

  /** 完整消息发送：可选前置设置对话模式 */
  async ask(question: string, mode?: "rag" | "search" | "chat") {
    if (mode) {
      await this.setMode(mode);
    }
    const composerInput = this.page.locator('[data-testid="workspace-chat-composer"]');
    await composerInput.waitFor({ timeout: 10_000 });
    await expect(composerInput).toBeEnabled({ timeout: 120_000 });
    await composerInput.click();
    await this.setComposerDraft(question);
    const sendButton = this.page.locator('[data-testid="workspace-chat-send"]');
    await expect(sendButton).toBeEnabled({ timeout: 10_000 });
    await sendButton.click();
  }

  async setMode(mode: "rag" | "search" | "chat") {
    await this.page.locator('[data-testid="workspace-chat-mode-button"]').click();
    await this.page.locator(`[data-testid="workspace-chat-mode-${mode}"]`).click();
  }

  async switchToWebSearchMode() {
    await this.setMode("search");
  }

  /**
   * 等待最后一条 assistant 消息的 data-pending 变为 false。
   * 适用于需要快速确认消息已渲染的场景（timeout 较短）。
   */
  async waitForResponse(timeout = 120_000) {
    await this.page
      .locator('[data-testid="chat-message"][data-role="assistant"][data-pending="false"]')
      .last()
      .waitFor({ timeout });
  }

  /**
   * 等待回答完整生成：先等 assistant 消息出现，再等 progress card 消失。
   * 适用于需要确认流式生成已结束的场景（timeout 较长）。
   */
  async waitForAnswer(timeoutMs = 120_000) {
    const assistantMessage = this.page
      .locator('[data-testid="chat-message"][data-role="assistant"]')
      .last();
    await assistantMessage.waitFor({ timeout: timeoutMs });
    try {
      await this.page
        .locator('[data-testid="workspace-progress-card"]')
        .waitFor({ state: "detached", timeout: timeoutMs });
    } catch {
      // progress card may not appear for fast responses
    }
  }

  getMessages(): Promise<Locator[]> {
    return this.page.locator("[data-testid='chat-message']").all();
  }

  getLastMessage(): Locator {
    return this.page.locator("[data-testid='chat-message']").last();
  }

  /**
   * 返回去除 HTML 标签的纯文本答案。
   * ⚠️ 测试 format-output（HTML/PPT）时，应使用 lastAnswerHtml() 获取原始 HTML。
   */
  async lastAnswerText(): Promise<string> {
    const text = await this.lastAnswerRenderedText();
    return text.trim();
  }

  /** 返回浏览器渲染后的可见文本（不含 HTML 标签） */
  async lastAnswerRenderedText(): Promise<string> {
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

  getCitationButton(): Locator {
    return this.page.locator("[data-testid='citation-button']").first();
  }

  async citationCount(): Promise<number> {
    const bubble = this.page
      .locator(
        '[data-testid="chat-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]',
      )
      .last();
    const workspace = await bubble.locator('[data-testid="workspace-citation"]').count();
    const webSources = await bubble.locator('[data-testid="citation-button"]').count();
    const htmlInline = await bubble.locator("button[data-inline-citation-token-index]").count();
    return workspace + webSources + htmlInline;
  }

  /** Web search may render inline citations or an aggregate "N sources" button. */
  async expectCitationUiVisible(timeoutMs = 10_000) {
    const bubble = this.page
      .locator(
        '[data-testid="chat-message"][data-role="assistant"] [data-testid="workspace-answer-bubble"]',
      )
      .last();
    const inlineCitation = bubble.locator('[data-testid="workspace-citation"]').first();
    const sourceButton = bubble.locator('[data-testid="citation-button"]').first();
    const inlineToken = bubble.locator("button[data-inline-citation-token-index]").first();

    if ((await inlineCitation.count()) > 0) {
      await expect(inlineCitation).toBeVisible({ timeout: timeoutMs });
      return;
    }
    if ((await sourceButton.count()) > 0) {
      await expect(sourceButton).toBeVisible({ timeout: timeoutMs });
      return;
    }
    if ((await inlineToken.count()) > 0) {
      await expect(inlineToken).toBeVisible({ timeout: timeoutMs });
      return;
    }

    throw new Error("expected at least one citation UI element in the last assistant bubble");
  }
}
