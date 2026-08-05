import { describe, expect, it } from "vitest";

import { sanitizeAssistantDisplayContent } from "../../hooks/chat-session/helpers";

describe("sanitizeAssistantDisplayContent", () => {
  it("strips python code fences that leaked into the final answer", () => {
    const raw = `你好\n\n\`\`\`python\nawait client.weather_query(city="北京")\n\`\`\`\n\n今天晴。`;
    expect(sanitizeAssistantDisplayContent(raw)).toBe("你好\n\n今天晴。");
  });

  it("strips double-backtick fences (broken fences from the model)", () => {
    const raw = `\`\`python\nimport asyncio\n\nctx = await client.user_context()\nprint(ctx)\n\`\``;
    expect(sanitizeAssistantDisplayContent(raw).trim()).toBe("");
  });

  it("strips implementation confession about wrong client APIs", () => {
    const raw =
      "抱歉，我错误地调用了 `client.weather_data()` 这个不存在的函数——正确的调用方式是 `client.weather_query(...)`。\n\n今天日期：2026-08-05";
    const out = sanitizeAssistantDisplayContent(raw);
    expect(out).not.toMatch(/weather_data|调用方式/);
    expect(out).toMatch(/今天日期/);
  });

  it("strips code_execution_result shells", () => {
    const raw = `结论\n<code_execution_result>{"ok":true}</code_execution_result>`;
    expect(sanitizeAssistantDisplayContent(raw)).toBe("结论");
  });

  it("still blanks pure tool JSON dumps", () => {
    expect(sanitizeAssistantDisplayContent(JSON.stringify({ tool: "weather", chunks: [] }))).toBe(
      "",
    );
  });
});
