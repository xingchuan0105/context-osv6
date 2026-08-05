import { describe, expect, it } from "vitest";

import { sanitizeAssistantDisplayContent } from "../../hooks/chat-session/helpers";

describe("sanitizeAssistantDisplayContent", () => {
  it("strips python code fences that leaked into the final answer", () => {
    const raw = `你好\n\n\`\`\`python\nawait client.weather_query(city="北京")\n\`\`\`\n\n今天晴。`;
    expect(sanitizeAssistantDisplayContent(raw)).toBe("你好\n\n今天晴。");
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
