import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";

test.describe("Session history", () => {
  test("messages survive page refresh", async ({ page }) => {
    const chat = new ChatPage(page);
    await chat.goto();
    await chat.ask("What is antifragility?");
    const before = await chat.lastAnswer();
    expect(before.length).toBeGreaterThan(20);

    await page.reload();
    await page.waitForLoadState("networkidle");

    const after = await chat.lastAnswer();
    expect(after).toContain(before.slice(0, 30)); // approximate match
  });
});
