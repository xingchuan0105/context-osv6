import { describe, expect, it } from "vitest";

import { sanitizeAssistantDisplayContent } from "../../hooks/chat-session/helpers";

/**
 * Fixtures stay structural/synthetic — no product copy, no real tool catalogues,
 * no realistic user confessions (repo prompt / golden-set rules).
 */
describe("sanitizeAssistantDisplayContent", () => {
  it("strips triple-backtick fences and keeps surrounding prose", () => {
    const raw = "alpha\n\n```lang\nawait client.foo()\n```\n\nbeta";
    expect(sanitizeAssistantDisplayContent(raw)).toBe("alpha\n\nbeta");
  });

  it("strips double-backtick fences", () => {
    const raw = "``lang\nimport x\nawait client.foo()\nprint(1)\n``";
    expect(sanitizeAssistantDisplayContent(raw).trim()).toBe("");
  });

  it("strips inline client.* tokens without language-specific heuristics", () => {
    const raw = "prefix `client.foo_bar()` mid client.baz suffix";
    const out = sanitizeAssistantDisplayContent(raw);
    expect(out).not.toMatch(/client\./);
    expect(out).toMatch(/prefix/);
    expect(out).toMatch(/suffix/);
  });

  it("strips host observation tag shells by structure", () => {
    const raw = 'keep\n<code_execution_result>{"ok":true}</code_execution_result>';
    expect(sanitizeAssistantDisplayContent(raw)).toBe("keep");
  });

  it("blanks whole-message tool-shaped JSON by keys", () => {
    expect(sanitizeAssistantDisplayContent(JSON.stringify({ tool: "x", chunks: [] }))).toBe("");
  });
});
