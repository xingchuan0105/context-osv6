import { test, expect } from "@playwright/test";
import { ChatPage } from "../src/pages/ChatPage";

test.describe.serial("Chat multi-turn session", () => {
  test("second turn references first turn context", async ({ page }) => {
    const chat = new ChatPage(page);
    await chat.goto();

    await chat.ask("What is antifragility?");
    const answer1 = await chat.lastAnswer();
    expect(answer1.length).toBeGreaterThan(20);

    await chat.ask("Who wrote the book about it?");
    const answer2 = await chat.lastAnswer();
    expect(answer2.toLowerCase()).toContain("taleb");
  });
});
